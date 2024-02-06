use std::time::Duration;

use crate::{analyse, observe, schedule::analysis_timer::class_budget::SelectedPrefix};

use self::{as_budget::AsBudgets, class_budget::ClassBudget};

use super::Params;
use anyhow::*;
use db_model::prefix_tree::PriorityClass;
use diesel::PgConnection;
use log::{debug, error, info, warn};
use prefix_crab::loop_with_stop;
use queue_models::probe_request::{EchoProbeRequest, ProbeRequest};
use strum::IntoEnumIterator;
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

        for prio in PriorityClass::iter() {
            observe::record_budget(prio, budgets.get_initial_available(prio), budgets.get_allocated(prio) as u64);
        }

        if budgets.is_empty() {
            warn!("No priority classes received any probe budget, are there leaves available?");
            return Ok(());
        }

        let start = Instant::now();
        let mut prefix_count = 0u32;
        for budget in budgets {
            let class = budget.class;
            prefix_count += self
                .process_class(&mut conn, budget, &mut as_budgets)
                .await
                .with_context(|| format!("processing class {:?}", class))?;
        }

        info!(
            "{} prefix analyses scheduled by timer in {}ms.",
            prefix_count,
            start.elapsed().as_millis(),
        );

        Ok(())
    }

    async fn process_class(
        &self,
        conn: &mut PgConnection,
        budget: ClassBudget,
        as_budgets: &mut AsBudgets,
    ) -> Result<u32> {
        let mut available_allocation = budget.allocated;
        let mut suppressed_count = 0u32;
        for _ in 0..5 {
            suppressed_count = 0; // reset since this represents slots that remain open _only_ due to AS rate limit, and not just "no prefixes available"
            let mut admitted_prefixes = vec![];

            for SelectedPrefix { net, asn } in budget.select_prefixes(conn, as_budgets)? {
                if available_allocation == 0 {
                    break;
                } else if as_budgets.try_consume(asn) {
                    admitted_prefixes.push(net);
                    available_allocation -= 1;
                } else {
                    suppressed_count += 1;
                }
            }

            analyse::persist::begin_bulk(conn, &admitted_prefixes)
                .context("saving analyses to begin")?;

            for target_net in admitted_prefixes {
                let req = EchoProbeRequest { target_net };
                if self.probe_tx.send(ProbeRequest::Echo(req)).await.is_err() {
                    info!("Receiver closed probe channel, assume shutdown.");
                    return Ok(budget.allocated - available_allocation);
                }
            }

            if suppressed_count == 0 || available_allocation == 0 {
                // don't try getting new prefixes if we have nothing left, or if we consumed everything we got,
                // implying there just aren't any more prefixes left that don't exceed the rate limit
                break;
            }
        }
        if suppressed_count > 0 {
            debug!(
                "Unable to fill class {:?} even after 5 retries, {} slots still suppressed",
                budget.class, suppressed_count
            );
        }
        Ok(budget.allocated - available_allocation)
    }
}
