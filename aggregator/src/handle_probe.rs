use anyhow::*;
use clap::Args;
use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use log::{debug, error, info, trace};
use tokio::sync::mpsc::{Receiver, UnboundedSender};

use prefix_crab::helpers::rabbit::ack_sender::CanAck;
use queue_models::echo_response::EchoProbeResponse;

use crate::models::path::PrefixPath;

mod interpret;
mod context;
mod archive;
mod store;

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
    connection: &mut PgConnection, ack_tx: &UnboundedSender<TaskRequest>, req: TaskRequest
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

fn handle_one(connection: &mut PgConnection, req: &TaskRequest) -> Result<(), Error> {
    let target_net: PrefixPath = req.model.target_net.into();
    debug!("Resolved path is {}", target_net);

    archive::process(connection, &target_net, &req.model);

    let context = context::fetch(connection, &target_net)
        .with_context(|| "while fetching context")?;

    let interpretation = interpret::process_echo(&req.model);

    info!("Context for this probe: {:?}", context);
    info!("Interpretation for this probe: {:?}", interpretation);

    Ok(())
}
