use anyhow::*;
use clap::Args;
use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use log::{debug, error, info, trace};
use tokio::sync::mpsc::{Receiver, UnboundedSender};

use prefix_crab::helpers::rabbit::ack_sender::CanAck;
use queue_models::echo_response::EchoProbeResponse;

use crate::analyse::context::{ContextFetchError, ContextFetchResult};
use crate::analyse::persist::UpdateAnalysis;
use crate::analyse::CanFollowUp;

use crate::{analyse, prefix_tree};

#[derive(Args, Debug)]
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
    let mut connection = connect_and_migrate_schema(&params)?;
    info!("Probe handler up & running!");
    loop {
        if let Some(req) = task_rx.recv().await {
            trace!("Received something: {:?}", req);
            handle_recv(&mut connection, &ack_tx, req)?;
        } else {
            info!("Probe handler shutting down.");
            return Ok(());
        }
    }
}

fn connect_and_migrate_schema(params: &Params) -> Result<PgConnection, Error> {
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
    Ok(connection)
}

fn handle_recv(
    connection: &mut PgConnection,
    ack_tx: &UnboundedSender<TaskRequest>,
    req: TaskRequest,
) -> Result<()> {
    match handle_one(connection, &req) {
        Result::Ok(_) => ack_tx.send(req).map_err(Error::msg),
        Err(e) => {
            // TODO Could be handled with DLQ
            error!("Failed to handle request: {:?} - shutting down.", req);
            Err(e)
        }
    }
}

fn handle_one(conn: &mut PgConnection, req: &TaskRequest) -> Result<(), Error> {
    let target_net = req.model.target_net;
    debug!("Resolved path is {}", target_net);

    archive::process(conn, &target_net, &req.model);

    let tree_context =
        prefix_tree::context::fetch(conn, &target_net).context("fetching tree context")?;
    let mut context =
        fetch_or_begin_context(conn, tree_context).context("fetch/begin for probe handling")?;

    let interpretation = analyse::echo::process(&req.model);

    info!("Context for this probe: {:?}", context);
    info!("Interpretation for this probe: {}", interpretation);

    // TODO schedule follow-ups now & here (still save if f/u fails!)
    let need_follow_up = interpretation.needs_follow_up();

    interpretation
        .update_analysis(conn, &mut context)
        .context("while saving analysis data")?;

    if !need_follow_up {
        info!("No further follow-up necessary, scheduling split analysis.");
        analyse::split::process(conn, context)?;
    } else {
        debug!("Follow-up needed, split analysis delayed.");
    }

    Ok(())
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

    use queue_models::echo_response::EchoProbeResponse;

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
