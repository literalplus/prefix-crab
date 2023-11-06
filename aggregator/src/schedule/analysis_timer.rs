use std::time::Duration;

use crate::analyse;

use super::Params;
use anyhow::*;
use log::{error, info, trace, warn};
use prefix_crab::loop_with_stop;
use queue_models::probe_request::{EchoProbeRequest, ProbeRequest};
use tokio::{
    sync::mpsc::UnboundedSender,
    time::{interval, Instant},
};
use tokio_util::sync::CancellationToken;

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
            trigger.tick() => self.tick() as simple
        )
    }

    fn tick(&mut self) {
        if let Err(e) = self.do_tick() {
            error!("Failed to schedule timed analysis due to {:?}", e);
        }
    }

    fn do_tick(&mut self) -> Result<()> {
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
            trace!(
                "Requesting probes for {} prefixes of prio {:?}",
                prefixes.len(),
                prio
            );
            prefix_count += prefixes.len();

            analyse::persist::begin_bulk(&mut conn, &prefixes)
                .context("saving analyses to begin")?;

            for target_net in prefixes {
                let req = EchoProbeRequest { target_net };
                if self.probe_tx.send(ProbeRequest::Echo(req)).is_err() {
                    info!("Receiver closed probe channel, assume shutdown.");
                    return Ok(());
                }
            }
        }

        info!(
            "{} prefix analyses scheduled by timer in {}ms.",
            prefix_count,
            start.elapsed().as_millis()
        );
        Ok(())
    }
}
