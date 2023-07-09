use anyhow::*;
use clap::Args;
use diesel::insert_into;
use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use log::{debug, error, info, trace};
use tokio::sync::mpsc::{Receiver, UnboundedSender};

use prefix_crab::helpers::rabbit::ack_sender::CanAck;
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
    params: Params
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

    insert_if_new(connection, &target_net)?;

    let parents = select_parents(connection, &target_net)?;
    info!("Parents: {:?}", parents);
    Ok(())
}

fn select_parents(
    connection: &mut PgConnection, target_net: &PrefixPath
) -> Result<Vec<PrefixTree>> {
    let parents = prefix_tree
        .filter(path.ancestor_or_same_as(target_net))
        .select(PrefixTree::as_select())
        .load(connection)
        .with_context(|| "while selecting parents")?;
    Ok(parents)
}

fn insert_if_new(connection: &mut PgConnection, target_net: &PrefixPath) -> Result<(), Error> {
    let inserted_id_or_zero = insert_into(prefix_tree).values((
        path.eq(target_net),
        is_routed.eq(true),
        merge_status.eq(MergeStatus::NotMerged),
        data.eq(ExtraData { ever_responded: true }),
    ))
        .on_conflict_do_nothing()
        .returning(id)
        .execute(connection)
        .with_context(|| "while trying to insert into prefix_tree")?;
    info!("ID for this prefix is {}.", inserted_id_or_zero);
    Ok(())
}
