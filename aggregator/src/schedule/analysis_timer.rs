use std::time::Duration;

use anyhow::*;
use log::{error, info, warn, trace};
use prefix_crab::loop_with_stop;
use queue_models::probe_request::{ProbeRequest, EchoProbeRequest};
use tokio::{
    sync::mpsc::UnboundedSender,
    time::{interval, Instant},
};
use tokio_util::sync::CancellationToken;
use super::Params;

mod class_budget;

pub async fn run(
    probe_tx: UnboundedSender<ProbeRequest>,
    stop_rx: CancellationToken,
    params: Params,
) -> Result<()> {
    Timer { probe_tx, params }.run(stop_rx).await
}

struct Timer {
    probe_tx: UnboundedSender<ProbeRequest>,
    params: Params,
}

impl Timer {
    async fn run(mut self, stop_rx: CancellationToken) -> Result<()> {
        info!("Analysis timer is ready for work.");
        let mut trigger = interval(Duration::from_secs(
            self.params.analysis_timer_interval_secs,
        ));
        loop_with_stop!(
            "analysis timer", stop_rx,
            trigger.tick() => tick(it) on self as simple
        )
    }

    async fn tick(&mut self, _it: Instant) -> Result<()> {
        match self.do_tick().await {
            Err(e) => {
                error!("Failed to schedule timed analysis due to {:?}", e);
                Ok(())
            }
            ok => ok,
        }
    }

    async fn do_tick(&mut self) -> Result<()> {
        let mut conn = crate::persist::connect()?;
        let budgets = class_budget::allocate(&mut conn, self.params.analysis_timer_prefix_budget)?;

        if budgets.is_empty() {
            warn!("No priority classes received any probe budget, are there leaves available?");
            return Ok(());
        }

        let start = Instant::now();
        let mut prefix_count = 0;
        for budget in budgets {
            let prio = budget.class;
            let prefixes = budget.select_prefixes(&mut conn)?;
            trace!("Requesting probes for {} prefixes of prio {:?}", prefixes.len(), prio);
            prefix_count += prefixes.len();

            // TODO space out a bit maybe? or anyways doesn't matter due to downstream batching?
            for target_net in prefixes {
                let req = EchoProbeRequest {
                    target_net,
                };
                if let Err(_) = self.probe_tx.send(ProbeRequest::Echo(req)) {
                    info!("Receiver closed probe channel, assume shutdown.");
                    return Ok(());
                }
            }
        }

        info!("{} prefix analyses scheduled by timer in {}ms.", prefix_count, start.elapsed().as_millis());
        Ok(())
    }
}
