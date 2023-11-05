use std::net::Ipv6Addr;

use anyhow::{Context, Result};
use itertools::Itertools;
use log::{debug, info, trace};
use prefix_crab::blocklist::{self, PrefixBlocklist};
use queue_models::{
    probe_request::TraceRequest,
    probe_response::{LastHop, TraceResponse, TraceResult},
};

use crate::{
    probe_store::{ProbeStore, RequestGroup, Target},
    yarrp_call::{Caller, TargetCollector},
};

use super::{TaskRequest, TaskResponse};

pub struct SchedulerTask {
    store: ProbeStore,
    caller: Caller,
    targets: TargetCollector,
    blocklist: PrefixBlocklist,
}

impl SchedulerTask {
    pub fn new(params: super::Params) -> Result<Self> {
        Ok(Self {
            store: ProbeStore::default(),
            caller: params.base.to_caller_assuming_sudo()?,
            targets: TargetCollector::new_default()?,
            blocklist: blocklist::read(params.blocklist)?,
        })
    }

    pub fn push_work(&mut self, mut item: TaskRequest) -> Result<()> {
        self.push_work_internal(&mut item)
            .with_context(|| format!("for request: {:?}", item))
    }

    fn push_work_internal(&mut self, item: &mut TaskRequest) -> Result<()> {
        self.apply_blocklist(&mut item.model);
        self.targets.push_slice(&item.model.targets)?;
        self.store.request_all(item);
        Ok(())
    }

    fn apply_blocklist(&self, item: &mut TraceRequest) {
        let predicate = |target: &Ipv6Addr| {
            if self.blocklist.is_blocked(target) {
                info!("[{}] Not tracing {:?} due to blocklist", item.id, target);
                false
            } else {
                true
            }
        };
        let filtered_targets = item.targets.clone().into_iter().filter(predicate).collect_vec();
        item.targets = filtered_targets;
    }

    pub async fn run(mut self) -> Result<Vec<TaskResponse>> {
        if !self.targets.is_empty() {
            let mut response_rx = self.caller.request_responses();
            self.targets.flush()?;
            let yarrp_task =
                tokio::task::spawn_blocking(move || self.caller.consume_run(self.targets));
            while let Some(response) = response_rx.recv().await {
                trace!("response from yarrp: {:?}", response);
                self.store.register_response(response);
            }
            response_rx.close(); // ensure nothing else is sent
            yarrp_task
                .await
                .with_context(|| "during blocking yarrp call (await)")??;
        } else {
            debug!("Skipping call, all requests of this chunk are empty.");
        }
        Ok(map_into_responses(self.store))
    }
}

fn map_into_responses(store: ProbeStore) -> Vec<TaskResponse> {
    store
        .into_request_groups()
        .into_iter()
        .map(map_into_response)
        .collect()
}

fn map_into_response(group: RequestGroup) -> TaskResponse {
    TaskResponse {
        acks_delivery_tag: group.delivery_tag,
        model: group.into(),
    }
}

impl From<RequestGroup> for TraceResponse {
    fn from(value: RequestGroup) -> Self {
        Self {
            id: value.request_id,
            results: value.targets.into_iter().map_into().collect(),
        }
    }
}

impl From<Target> for TraceResult {
    fn from(value: Target) -> Self {
        match value.last_hop {
            Some(hop) => TraceResult::LastResponsiveHop(LastHop {
                target_addr: value.addr,
                last_hop_addr: hop.addr,
                last_hop_ttl: hop.sent_ttl,
                target_ttl: value.target_own_ttl,
                response_type: hop.response_type,
            }),
            None => TraceResult::NoResponse {
                target_addr: value.addr,
            },
        }
    }
}
