use std::time::Duration;

use anyhow::*;
use diesel::prelude::*;
use diesel::PgConnection;
use ipnet::Ipv6Net;
use log::{error, info, warn};
use prefix_crab::loop_with_stop;
use queue_models::probe_request::ProbeRequest;
use tokio::{
    sync::mpsc::UnboundedSender,
    time::{interval, Instant},
};
use tokio_util::sync::CancellationToken;

use crate::prefix_tree::MergeStatus;
use crate::prefix_tree::PriorityClass;

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

        for budget in budgets {
            todo!()
        }

        Ok(())
    }
}
