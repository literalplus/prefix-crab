use std::sync::OnceLock;

use anyhow::*;
use diesel::{Connection, PgConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use log::{debug, info};
use tracing::instrument;
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

#[instrument(name = "DB connect")]
pub fn connect(app_name: &str) -> Result<PgConnection> {
    let params = STORED_PARAMS
        .get()
        .expect("params to be stored by initialisation call");
    connect_manual(app_name, params)
}

pub fn connect_manual(app_name: &str, params: &Params) -> Result<PgConnection> {
    let mut url = Url::parse(&params.database_url)?;
    url.query_pairs_mut()
        .append_pair("application_name", app_name);

    // Postgres expects queries percent-encoded, but url encodes them as application/x-form-www-urlencoded
    // The subtle difference is that spaces are encoded as + in the latter and %20 in the former
    let query_percent_encoded = url.query().unwrap_or("").replace('+', "%20");
    url.set_query(Some(&query_percent_encoded));

    PgConnection::establish(url.as_str()).with_context(|| "while connecting to Postgres")
}
