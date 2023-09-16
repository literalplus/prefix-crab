use std::time::Duration;

use anyhow::*;
use clap::Args;
use log::{info, error};
use prefix_crab::loop_with_stop;
use tokio::time::{interval, Instant};
use tokio_util::sync::CancellationToken;

#[derive(Args, Debug)]
#[group(id = "schedule")]
pub struct Params {
    /// How often to update AS data from the filesystem, in seconds (default = 6h).
    /// An immediate update can be triggered by restarting the application.
    #[arg(long, env = "RESEED_INTERVAL_SECS", default_value = "21600")]
    reseed_interval_secs: u64,
}

pub async fn run(stop_rx: CancellationToken, params: Params) -> Result<()> {
    info!("Automatic re-seed scheduled every {}s.", params.reseed_interval_secs);
    let mut trigger = interval(Duration::from_secs(params.reseed_interval_secs));
    loop_with_stop!(
        "analysis timer", stop_rx,
        trigger.tick() => tick(it) as simple
    )
}

async fn tick(_it: Instant) -> Result<()> {
    match do_tick().await {
        Err(e) => {
            error!("Failed to perform scheduled re-seed due to {:?}", e);
            Ok(())
        }
        ok => ok,
    }
}

async fn do_tick() -> Result<()> {
    let mut conn = crate::persist::connect()?;
    let start = Instant::now();

    info!(
        "Re-seed completed in {}ms.",
        start.elapsed().as_millis()
    );
    Ok(())
}
