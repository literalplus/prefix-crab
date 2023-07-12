use anyhow::*;
use clap::Args;
use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use log::{debug, error, info, trace};
use tokio::sync::mpsc::{Receiver, UnboundedSender};

use prefix_crab::helpers::rabbit::ack_sender::CanAck;
use queue_models::echo_response::EchoProbeResponse;

use crate::models::path::PrefixPath;

#[derive(Args)]
#[derive(Debug)]
#[group(id = "handler")]
pub struct Params {
    /// URI for PostgreSQL server to connect to
    /// Environment variable: DATABASE_URL
    /// If a password is required, it is recommended to specify the URL over the environment or
    /// a config file, to avoid exposure in shell history and process list.
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,
}

#[derive(Debug)]
pub struct TaskRequest {
    pub model: EchoProbeResponse,
    pub delivery_tag: u64,
}

impl CanAck for TaskRequest {
    fn delivery_tag(&self) -> u64 {
        self.delivery_tag
    }
}

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub async fn run(
    mut task_rx: Receiver<TaskRequest>,
    ack_tx: UnboundedSender<TaskRequest>,
    params: Params,
) -> Result<()> {
    let mut connection = PgConnection::establish(&params.database_url)
        .with_context(|| "While connecting to PostgreSQL")?;
    debug!("Running any pending migrations now.");
    match connection.run_pending_migrations(MIGRATIONS) {
        Result::Ok(migrations_run) => {
            for migration in migrations_run {
                info!("Schema migration run: {}", migration);
            }
        }
        Err(e) => Err(anyhow!(e)).with_context(|| "While running migrations")?,
    }
    info!("Probe handler up & running!");
    loop {
        if let Some(req) = task_rx.recv().await {
            trace!("Received something: {:?}", req);

            match handle_one(&mut connection, &req) {
                Result::Ok(_) => ack_tx.send(req)?,
                Err(e) => {
                    // TODO Could be handled with DLQ
                    error!("Failed to handle request: {:?} - shutting down.", req);
                    return Err(e);
                }
            }
        } else {
            info!("Probe handler shutting down.");
            return Ok(());
        }
    }
}

fn handle_one(connection: &mut PgConnection, req: &TaskRequest) -> Result<(), Error> {
    let target_net: PrefixPath = req.model.target_net.into();
    debug!("Resolved path is {}", target_net);

    archive::process(connection, &target_net, &req.model);

    let context = context::fetch(connection, &target_net)
        .with_context(|| "while fetching context")?;

    info!("Context for this probe: {:?}", context);

    Ok(())
}

mod interpret {
    use std::collections::HashMap;
    use std::iter::Peekable;
    use std::net::Ipv6Addr;
    use std::path::Iter;

    use ipnet::Ipv6Net;
    use itertools::Itertools;

    use queue_models::echo_response::{DestUnreachKind, EchoProbeResponse, ResponseKey, Responses, SplitResult};
    use queue_models::echo_response::ResponseKey::*;

    enum FollowUpTraceRequest {
        TraceResponsive { targets: Vec<Ipv6Addr>, sent_ttl: u8 },
        TraceUnresponsive { candidates: Vec<Ipv6Addr> },
    }

    enum LastHopRouterSource {
        TraceUnresponsive,
        TraceResponsive,
        DestinationUnreachable { kind: DestUnreachKind },
    }

    struct LastHopRouter {
        router: Ipv6Addr,
        handled_addresses: Vec<Ipv6Addr>,
        source: LastHopRouterSource,
    }

    enum WeirdBehaviourKind {
        TtlExceeded { sent_ttl: u8 },
        Other { description: String },
    }

    struct WeirdBehaviour {
        from: Ipv6Addr,
        kind: WeirdBehaviourKind,
    }

    struct SplitAnalysisResult {
        split: Ipv6Net,
        follow_ups: Vec<FollowUpTraceRequest>,
        last_hop_routers: Vec<LastHopRouter>,
        weird_behaviours: Vec<WeirdBehaviour>,
    }

    impl SplitAnalysisResult {
        fn new(split: &SplitResult) -> SplitAnalysisResult {
            Self {
                split: split.net,
                follow_ups: vec![],
                last_hop_routers: vec![],
                weird_behaviours: vec![],
            }
        }
    }

    pub fn process(model: &EchoProbeResponse) {
        let mut split_results = vec![];
        for split in model.splits {
            split_results.push(process_split(&split));
        }
        // TODO handle results of split processing
    }

    fn process_split(split: &SplitResult) {
        let mut result = SplitAnalysisResult::new(split);
        let mut some_addrs_were_unresponsive = false;
        for response in responses {
            match &response.key {
                DestinationUnreachable { kind, from } => {
                    result.last_hop_routers.push(LastHopRouter {
                        router: *from,
                        handled_addresses: response.intended_targets.clone(),
                        source: LastHopRouterSource::DestinationUnreachable { kind: *kind },
                    })
                }
                EchoReply { different_from: _, sent_ttl } => {
                    result.follow_ups.push(FollowUpTraceRequest::TraceResponsive {
                        targets: response.intended_targets.clone(),
                        sent_ttl: *sent_ttl,
                    })
                }
                Other { description } => {
                    for target in response.intended_targets {
                        result.weird_behaviours.push(WeirdBehaviour {
                            from: target,
                            kind: WeirdBehaviourKind::Other {
                                description: description.to_string(),
                            },
                        })
                    }
                }
                NoResponse => {
                    some_addrs_were_unresponsive = true;
                }
                TimeExceeded { from, sent_ttl } => {
                    result.weird_behaviours.push(WeirdBehaviour {
                        from: *from,
                        kind: WeirdBehaviourKind::TtlExceeded { sent_ttl: *sent_ttl },
                    })
                }
            }
        }
        let nothing_else_recorded = result.follow_ups.is_empty() &&
            result.last_hop_routers.is_empty();
        if some_addrs_were_unresponsive && nothing_else_recorded {
            // TODO
        }
    }
}

mod context {
    use anyhow::*;
    use diesel::dsl::not;
    use diesel::insert_into;
    use diesel::prelude::*;

    use crate::models::path::{PathExpressionMethods, PrefixPath};
    use crate::models::tree::*;
    use crate::schema::prefix_tree::dsl::*;

    #[derive(Debug)]
    pub struct ProbeContext {
        node: PrefixTree,
        ancestors: Vec<PrefixTree>,
        unmerged_children: Vec<PrefixTree>,
    }

    pub fn fetch(
        connection: &mut PgConnection, target_net: &PrefixPath,
    ) -> Result<ProbeContext> {
        insert_if_new(connection, &target_net)?;

        let ancestors_and_self = select_ancestors_and_self(connection, &target_net)
            .with_context(|| "while finding ancestors and self")?;
        let (ancestors, node) = match &ancestors_and_self[..] {
            [parents @ .., node] => (parents.to_vec(), *node),
            [] => bail!("Didn't find the prefix_tree node we just inserted :("),
        };
        let unmerged_children = select_unmerged_children(connection, &target_net)?;
        Result::Ok(ProbeContext { node, ancestors, unmerged_children })
    }

    fn select_ancestors_and_self(
        connection: &mut PgConnection, target_net: &PrefixPath,
    ) -> Result<Vec<PrefixTree>> {
        let parents = prefix_tree
            .filter(path.ancestor_or_same_as(target_net))
            .select(PrefixTree::as_select())
            .order_by(path)
            .load(connection)
            .with_context(|| "while selecting parents")?;
        Ok(parents)
    }

    fn select_unmerged_children(
        connection: &mut PgConnection, target_net: &PrefixPath,
    ) -> Result<Vec<PrefixTree>> {
        let parents = prefix_tree
            .filter(path.descendant_or_same_as(target_net))
            .filter(not(path.eq(target_net)))
            .select(PrefixTree::as_select())
            .order_by(path)
            .load(connection)
            .with_context(|| "while selecting unmerged children")?;
        Ok(parents)
    }

    fn insert_if_new(connection: &mut PgConnection, target_net: &PrefixPath) -> Result<(), Error> {
        let _inserted_id_or_zero = insert_into(prefix_tree)
            .values((
                path.eq(target_net),
                is_routed.eq(true),
                merge_status.eq(MergeStatus::NotMerged),
                data.eq(ExtraData { ever_responded: true }),
            ))
            .on_conflict_do_nothing()
            .returning(id)
            .execute(connection)
            .with_context(|| "while trying to insert into prefix_tree")?;
        Ok(())
    }
}

mod archive {
    use anyhow::*;
    use diesel::insert_into;
    use diesel::prelude::*;
    use log::{trace, warn};

    use queue_models::echo_response::EchoProbeResponse;

    use crate::models::path::PrefixPath;
    use crate::schema::response_archive::dsl::*;

    pub fn process(
        connection: &mut PgConnection, target_net: &PrefixPath, model: &EchoProbeResponse,
    ) {
        // Note: This could technically be separated into a different component, then that should
        // be independent of any processing errors (giving us a decent chance at reprocessing if
        // combined with some sort of success flag/DLQ
        if let Err(e) = archive_response(connection, &target_net, &model) {
            warn!("Unable to archive response: {:?} - due to {}", &model, e);
        } else {
            trace!("Response successfully archived.");
        }
    }

    fn archive_response(
        connection: &mut PgConnection, target_net: &PrefixPath, model: &EchoProbeResponse,
    ) -> Result<(), Error> {
        let model_jsonb = serde_json::to_value(model)
            .with_context(|| "failed to serialize to json for archiving")?;
        insert_into(response_archive)
            .values((
                path.eq(target_net),
                data.eq(model_jsonb),
            ))
            .execute(connection)
            .with_context(|| "while trying to insert into response archive")?;
        Ok(())
    }
}
