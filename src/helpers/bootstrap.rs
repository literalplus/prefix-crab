use std::time::Duration;

use anyhow::{Context, Result};
use human_panic::setup_panic;
use log::{debug, warn};

use crate::helpers::logging;

pub fn run<CliType>(
    fn_cli_parse: fn() -> CliType,
    fn_extract_logging: fn(&CliType) -> &logging::Params,
    fn_run: fn(CliType) -> Result<()>,
) -> Result<()> {
    setup_panic!();
    if let Err(env_err) = dotenvy::dotenv() {
        if env_err.not_found() {
            warn!("No `.env` file found (recursively). You usually want to have one.")
        } else {
            return Err(env_err).with_context(|| "Failed to load `.env` file");
        }
    }

    let cli = fn_cli_parse();
    let logger_handle = logging::configure_from(fn_extract_logging(&cli))?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .with_context(|| "Failed to start Tokio runtime")?;
    let _guard = runtime.enter();

    let command_result = fn_run(cli);

    debug!("Waiting up to 15 seconds for remaining tasks to finish");
    runtime.shutdown_timeout(Duration::from_secs(15));

    // Important with non-direct write mode
    // Handle needs to be kept alive until end of program
    logger_handle.flush();

    command_result
}
