use clap::Args;
pub use connect::*;
pub use error::*;
pub use loader::*;
pub use macros::configure_jsonb_serde;

pub mod dsl;

mod connect;
mod error;
mod loader;
mod macros;

#[derive(Args, Debug, Clone)]
#[group(id = "database")]
pub struct Params {
    /// URI for PostgreSQL server to connect to
    /// Environment variable: DATABASE_URL
    /// If a password is required, it is recommended to specify the URL over the environment or
    /// a config file, to avoid exposure in shell history and process list.
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,
}

impl Params {
    pub fn new(database_url: String) -> Self {
        Self { database_url }
    }
}
