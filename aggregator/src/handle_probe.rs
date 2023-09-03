use anyhow::*;
use diesel::prelude::*;
use ipnet::Ipv6Net;
use log::{debug, error, info, trace, warn};
use tokio::sync::mpsc::{Receiver, UnboundedSender};

use prefix_crab::helpers::rabbit::ack_sender::CanAck;
use queue_models::probe_response::EchoProbeResponse;

use crate::analyse::context::{self, ContextFetchError, ContextFetchResult};
use crate::analyse::persist::UpdateAnalysis;
use crate::analyse::{CanFollowUp, EchoResult};

use crate::analyse::split::SplitError;
use crate::prefix_tree::ContextOps;
use crate::schedule::FollowUpRequest;
use crate::{analyse, prefix_tree};

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

pub async fn run(
    task_rx: Receiver<TaskRequest>,
    ack_tx: UnboundedSender<TaskRequest>,
    follow_up_tx: UnboundedSender<FollowUpRequest>,
) -> Result<()> {
    let conn = crate::persist::connect()?;
    let handler = ProbeHandler {
        conn,
        ack_tx,
        follow_up_tx,
    };

    info!("Probe handler is ready to receive work!");
    handler.run(task_rx).await
}

struct ProbeHandler {
    conn: PgConnection,
    ack_tx: UnboundedSender<TaskRequest>,
    follow_up_tx: UnboundedSender<FollowUpRequest>,
}

impl ProbeHandler {
    async fn run(mut self, mut task_rx: Receiver<TaskRequest>) -> Result<()> {
        loop {
            if let Some(req) = task_rx.recv().await {
                trace!("Received something: {:?}", req);
                self.handle_recv(req).context("handling probe responses")?;
            } else {
                info!("Probe handler shutting down.");
                return Ok(());
            }
        }
    }

    fn handle_recv(&mut self, req: TaskRequest) -> Result<()> {
        match self.handle_one(&req) {
            Result::Ok(_) => self.ack_tx.send(req).map_err(Error::msg),
            Err(e) => {
                // TODO Could be handled with DLQ
                error!("Failed to handle request: {:?} - shutting down.", req);
                Err(e)
            }
        }
    }

    fn handle_one(&mut self, req: &TaskRequest) -> Result<(), Error> {
        let target_net = req.model.target_net;
        debug!("Resolved path is {}", target_net);

        archive::process(&mut self.conn, &target_net, &req.model);

        let (interpretation, context) = interpret_and_save(&mut self.conn, target_net, &req.model)?;

        if interpretation.needs_follow_up() {
            if let Some(id) = &context.analysis.pending_follow_up {
                let model = FollowUpRequest {
                    id: id.parse().context("Invalid TypeID stored in node")?,
                    prefix_tree: context.node().clone(),
                    follow_ups: interpretation.follow_ups,
                };
                info!("Sending follow-up {}, split analysis delayed.", model.id);
                self.follow_up_tx.send(model).context("sending follow-up")?;
            } else {
                warn!("Interpretation needs follow-up but it wasn't registered in the node");
            }
        } else {
            info!("No further follow-up necessary, scheduling split analysis.");
            match analyse::split::process(&mut self.conn, context) {
                Err(SplitError::PrefixNotLeaf { request }) => warn!(
                    "Handled prefix is (no longer?) a leaf, split not possible: {:?}",
                    request.node()
                ),
                r => r?,
            }
        }

        Ok(())
    }
}

fn interpret_and_save(
    conn: &mut PgConnection,
    target_net: Ipv6Net,
    model: &EchoProbeResponse,
) -> Result<(EchoResult, context::Context)> {
    let tree_context =
        prefix_tree::context::fetch(conn, &target_net).context("fetching tree context")?;
    let mut context = fetch_or_begin_context(conn, tree_context)
        .context("fetch/begin context for probe handling")?;

    let mut interpretation = analyse::echo::process(model);

    interpretation
        .update_analysis(conn, &mut context)
        .context("while saving analysis data")?;

    Ok((interpretation, context))
}

fn fetch_or_begin_context(
    conn: &mut PgConnection,
    parent: prefix_tree::context::Context,
) -> ContextFetchResult {
    let result = analyse::context::fetch(conn, parent);
    if let Err(ContextFetchError::NoActiveAnalysis { parent }) = result {
        // TODO probably shouldn't tolerate this any more once we actually create these analyses
        return analyse::persist::begin(conn, parent);
    }
    result
}

mod archive {
    use anyhow::*;
    use diesel::insert_into;
    use diesel::prelude::*;
    use ipnet::Ipv6Net;
    use log::{trace, warn};

    use queue_models::probe_response::EchoProbeResponse;

    use crate::persist::dsl::CidrMethods;
    use crate::schema::response_archive::dsl::*;

    pub fn process(conn: &mut PgConnection, target_net: &Ipv6Net, model: &EchoProbeResponse) {
        // Note: This could technically be separated into a different component, then that should
        // be independent of any processing errors (giving us a decent chance at reprocessing if
        // combined with some sort of success flag/DLQ)
        if let Err(e) = archive_response(conn, target_net, model) {
            warn!("Unable to archive response: {:?} - due to {}", &model, e);
        } else {
            trace!("Response successfully archived.");
        }
    }

    fn archive_response(
        conn: &mut PgConnection,
        target_net: &Ipv6Net,
        model: &EchoProbeResponse,
    ) -> Result<(), Error> {
        let model_jsonb = serde_json::to_value(model)
            .with_context(|| "failed to serialize to json for archiving")?;
        insert_into(response_archive)
            .values((path.eq6(target_net), data.eq(model_jsonb)))
            .execute(conn)
            .with_context(|| "while trying to insert into response archive")?;
        Ok(())
    }
}
