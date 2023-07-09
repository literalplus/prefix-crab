use anyhow::*;
use clap::Args;
use diesel::insert_into;
use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use log::{debug, info, trace};
use tokio::sync::mpsc::Receiver;

use queue_models::echo_response::EchoProbeResponse;

use crate::models::prefix_tree::*;
use crate::schema::prefix_tree::dsl::*;

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
}

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub async fn run(mut task_rx: Receiver<TaskRequest>, params: Params) -> Result<()> {
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
            let target_net: PrefixPath = req.model.target_net.into();
            insert_into(prefix_tree).values((
                path.eq(target_net),
                is_routed.eq(true),
                merge_status.eq(MergeStatus::NotMerged),
                data.eq(ExtraData { ever_responded: true }),
            )).execute(&mut connection)
                .with_context(|| "while trying to insert into prefix_tree")?;
        } else {
            info!("Probe handler shutting down.");
            return Ok(());
        }
    }
}
