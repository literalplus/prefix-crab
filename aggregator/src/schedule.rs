use anyhow::*;
use clap::Args;
use queue_models::probe_request::ProbeRequest;
use tokio::{
    sync::mpsc::{Receiver, Sender},
    try_join,
};
use tokio_util::sync::CancellationToken;

use crate::flatten;

mod analysis_timer;
mod follow_up;

pub use follow_up::FollowUpRequest;

#[derive(Args, Debug)]
#[group(id = "schedule")]
pub struct Params {
    /// How often to trigger a new analysis batch, in seconds
    #[arg(long, env = "ANALYSIS_TIMER_INTERVAL_SECS", default_value = "30")]
    analysis_timer_interval_secs: u64,

    /// How many prefixes to scan per timer interval
    #[arg(long, env = "ANALYSIS_TIMER_PREFIX_BUDGET", default_value = "20")]
    analysis_timer_prefix_budget: u32,

    /// How many prefixes to allow at most for one AS, per schedule
    /// Default value 223 ~= 75 pps
    /// (common ICMP rate limit, if the AS were served by a single router, plus 25 pps margin)
    #[arg(long, env = "ANALYSIS_TIMER_MAX_PREFIX_PER_AS", default_value = "223")]
    analysis_timer_max_prefix_per_as: usize,

    /// Whether to run the regular prefix schedule, or not (disabling the entire feedback system eventually)
    #[arg(long, env = "AGG_DO_SCHEDULE", default_value = "true")]
    do_schedule: bool,
}

pub async fn run(
    probe_tx: Sender<ProbeRequest>,
    follow_up_rx: Receiver<FollowUpRequest>,
    stop_rx: CancellationToken,
    params: Params,
) -> Result<()> {
    let follow_up_handle = tokio::spawn(follow_up::run(probe_tx.clone(), follow_up_rx));
    let timer_handle = tokio::spawn(analysis_timer::run(probe_tx, stop_rx, params));

    try_join!(flatten(follow_up_handle), flatten(timer_handle))?;
    Ok(())
}
