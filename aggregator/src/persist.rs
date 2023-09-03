use clap::Args;
pub use error::*;
pub use loader::*;
pub(crate) use macros::configure_jsonb_serde;
pub use connect::*;

pub mod dsl;
pub mod schema;

mod error;
mod loader;
mod macros;
mod connect;

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
