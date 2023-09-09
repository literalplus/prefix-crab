use anyhow::*;
use queue_models::probe_request::ProbeRequest;
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    try_join,
};

use crate::flatten;

mod follow_up;

pub use follow_up::FollowUpRequest;

pub async fn run(
    probe_tx: UnboundedSender<ProbeRequest>,
    follow_up_rx: UnboundedReceiver<FollowUpRequest>,
) -> Result<()> {
    let follow_up_handle = tokio::spawn(follow_up::run(probe_tx, follow_up_rx));

    try_join!(flatten(follow_up_handle))?;
    Ok(())
}

mod analysis_timer {
    use std::time::Duration;

    use log::info;
    use prefix_crab::loop_with_stop;
    use queue_models::probe_request::ProbeRequest;
    use tokio::{sync::mpsc::UnboundedSender, time::{interval, Instant}};
    use anyhow::*;
    use tokio_util::sync::CancellationToken;

    pub async fn run(
        probe_tx: UnboundedSender<ProbeRequest>,
        stop_rx: CancellationToken,
    ) -> Result<()> {
        info!("Analysis timer is ready for work.");
        let mut trigger = interval(Duration::from_secs(60));
        loop_with_stop!(
            "analysis timer", stop_rx,
            trigger.tick() => handle(it) as simple
        )
    }

    async fn handle(it: Instant) -> Result<()> {
        Ok(())
    }
}
