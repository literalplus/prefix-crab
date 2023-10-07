use std::{path::PathBuf, time::Duration};

use anyhow::*;
use clap::Args;
use log::{error, info};
use prefix_crab::loop_with_stop;
use tokio::time::{interval, Instant};
use tokio_util::sync::CancellationToken;

use crate::as_changeset::{self, AsChangeset};

#[derive(Args, Debug, Clone)]
#[group(id = "schedule")]
pub struct Params {
    /// How often to update AS data from the filesystem, in seconds (default = 6h).
    /// An immediate update can be triggered by restarting the application.
    #[arg(long, env = "RESEED_INTERVAL_SECS", default_value = "21600")]
    reseed_interval_secs: u64,

    #[arg(long, env = "AS_REPO_BASE_DIR", default_value = "./asn-ip/as")]
    as_repo_base_dir: PathBuf,

    /// Whether to insert freshly seeded prefixes into the tree for analysis.
    /// Note that the default value of 'false' means that NO internet-wide scan
    /// is performed, and only prefixes that are manually added to the tree are
    /// processed.
    #[arg(long, env = "PUSH_FRESH_PREFIXES_TO_TREE", default_value = "false")]
    push_fresh_prefixes_to_tree: bool,
}

pub async fn run(stop_rx: CancellationToken, params: Params) -> Result<()> {
    if !params
        .as_repo_base_dir
        .metadata()
        .map(|meta| meta.is_dir())
        .context("checking AS repo base dir")?
    {
        return Err(anyhow!(
            "AS repo base dir {:?} is not a directory",
            params.as_repo_base_dir
        ));
    }
    info!(
        "Automatic re-seed scheduled every {}s.",
        params.reseed_interval_secs
    );
    let mut trigger = interval(Duration::from_secs(params.reseed_interval_secs));
    loop_with_stop!(
        "analysis timer", stop_rx,
        trigger.tick() => tick((&params)) as simple
    )
}

fn tick(params: &Params) {
    if let Err(e) = do_tick(params) {
        error!("Failed to perform scheduled re-seed due to {:?}", e);
    }
}

fn do_tick(params: &Params) -> Result<()> {
    let mut conn = crate::persist::connect()?;
    let start = Instant::now();

    let changes = as_changeset::determine(&mut conn, &params.as_repo_base_dir)
        .context("determining AS set")?;

    info!("AS Changeset: {:?}", changes);

    info!("Re-seed completed in {}ms.", start.elapsed().as_millis());
    Ok(())
}

fn save_changes(changes: AsChangeset) -> Result<()> {


    Ok(())
}
