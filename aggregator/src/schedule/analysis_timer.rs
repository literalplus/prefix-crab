use std::time::Duration;

use crate::{analyse, schedule::analysis_timer::class_budget::SelectedPrefix};

use super::Params;
use anyhow::*;
use log::{error, info, trace, warn};
use prefix_crab::loop_with_stop;
use queue_models::probe_request::{EchoProbeRequest, ProbeRequest};
use tokio::{
    sync::mpsc::Sender,
    time::{interval, Instant},
};
use tokio_util::sync::CancellationToken;

mod as_budget;
mod class_budget;

pub async fn run(
    probe_tx: Sender<ProbeRequest>,
    stop_rx: CancellationToken,
    params: Params,
) -> Result<()> {
    Timer { probe_tx, params }.run(stop_rx).await
}

struct Timer {
    probe_tx: Sender<ProbeRequest>,
    params: Params,
}

impl Timer {
    async fn run(mut self, stop_rx: CancellationToken) -> Result<()> {
        if !self.params.do_schedule {
            warn!("Regular scheduling is disabled, not doing that.");
            return Ok(());
        }
        info!("Analysis timer is ready for work.");
        let mut trigger = interval(Duration::from_secs(
            self.params.analysis_timer_interval_secs,
        ));
        loop_with_stop!(
            "analysis timer", stop_rx,
            trigger.tick() => self.tick() as void_async
        )
    }

    async fn tick(&mut self) {
        if let Err(e) = self.do_tick().await {
            error!("Failed to schedule timed analysis due to {:?}", e);
        }
    }

    async fn do_tick(&mut self) -> Result<()> {
        let mut conn = crate::persist::connect("aggregator - analysis timer")?;
        let mut as_budgets = as_budget::allocate(&self.params);
        let budgets = class_budget::allocate(&mut conn, self.params.analysis_timer_prefix_budget)?;

        if budgets.is_empty() {
            warn!("No priority classes received any probe budget, are there leaves available?");
            return Ok(());
        }

        let start = Instant::now();
        let mut prefix_count = 0;
        let mut suppressed = 0;
        for budget in budgets {
            // TODO: If suppression has too high impact in practice, consider "retry" inside
            // a single budget to enable full allocation. As it is implemented here, if an
            // AS's budget is exhausted inside a single class budget, any remaining prefixes
            // are just suppressed, which might be an issue if the workload is heavily skewed
            // toward a single AS. The result is that fewer prefixes are probed than the budget.

            let prio = budget.class;
            let prefixes = budget.select_prefixes(&mut conn, &as_budgets)?;
            trace!(
                "Allocated probes for {} prefixes of prio {:?}",
                prefixes.len(),
                prio
            );

            let mut admitted_prefixes = vec![];
            for SelectedPrefix { net, asn } in prefixes {
                if as_budgets.try_consume(asn) {
                    admitted_prefixes.push(net);
                    prefix_count += 1;
                } else {
                    suppressed += 1;
                }
            }

            analyse::persist::begin_bulk(&mut conn, &admitted_prefixes)
                .context("saving analyses to begin")?;

            for target_net in admitted_prefixes {
                let req = EchoProbeRequest { target_net };
                if self.probe_tx.send(ProbeRequest::Echo(req)).await.is_err() {
                    info!("Receiver closed probe channel, assume shutdown.");
                    return Ok(());
                }
            }
        }

        info!(
            "{} prefix analyses scheduled by timer in {}ms{}.",
            prefix_count,
            start.elapsed().as_millis(),
            if suppressed > 0 {
                format!(" ({} suppressed by AS-level rate limit)", suppressed)
            } else {
                "".to_string()
            }
        );

        Ok(())
    }
}
