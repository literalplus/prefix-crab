use std::sync::OnceLock;

use anyhow::*;
use diesel::{Connection, PgConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use log::{debug, info};
use url::Url;

use super::Params;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
static STORED_PARAMS: OnceLock<Params> = OnceLock::new();

/// Must only be called once, and that is from main.rs
pub fn initialize(params: &Params) -> Result<()> {
    STORED_PARAMS
        .set(params.clone())
        .expect("not to be initialised already");
    let mut conn = connect("schema_migration")?;

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

pub fn connect(app_name: &str) -> Result<PgConnection> {
    let params = STORED_PARAMS
        .get()
        .expect("params to be stored by initialisation call");

    let mut url = Url::parse(&params.database_url)?;
    url.query_pairs_mut()
        .append_pair("application_name", app_name);

    PgConnection::establish(url.as_str()).with_context(|| "while connecting to Postgres")
}
