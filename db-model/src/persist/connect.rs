use std::sync::OnceLock;

use anyhow::*;
use diesel::{Connection, PgConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use log::{debug, info};

use super::Params;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
static STORED_PARAMS: OnceLock<Params> = OnceLock::new();

/// Must only be called once, and that is from main.rs
pub fn initialize(params: &Params) -> Result<()> {
    STORED_PARAMS.set(params.clone()).expect("not to be initialised already");
    let mut conn = connect()?;

    debug!("Running any pending migrations now.");
    match conn.run_pending_migrations(MIGRATIONS) {
        Result::Ok(migrations_run) => {
            for migration in migrations_run {
                info!("Schema migration run: {}", migration);
            }
        }
        Err(e) => Err(anyhow!(e)).with_context(|| "While running Postgres migrations")?,
    }
    Ok(())
}

pub fn connect() -> Result<PgConnection> {
    let params = STORED_PARAMS.get().expect("params to be stored by initialisation call");
    PgConnection::establish(&params.database_url).with_context(|| "while connecting to Postgres")
}
