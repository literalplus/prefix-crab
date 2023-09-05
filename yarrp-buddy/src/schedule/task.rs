use anyhow::{Context, Result};
use itertools::Itertools;
use log::trace;
use queue_models::probe_response::{LastHop, TraceResponse, TraceResult};

use crate::{
    probe_store::{ProbeStore, RequestGroup, Target},
    yarrp_call::{self, Caller, TargetCollector},
};

use super::{TaskRequest, TaskResponse};

pub struct SchedulerTask {
    store: ProbeStore,
    caller: Caller,
    targets: TargetCollector,
}

impl SchedulerTask {
    pub fn new(zmap_params: yarrp_call::Params) -> Result<Self> {
        Ok(Self {
            store: ProbeStore::default(),
            caller: zmap_params.to_caller_assuming_sudo()?,
            targets: TargetCollector::new_default()?,
        })
    }

    pub fn push_work(&mut self, item: TaskRequest) -> Result<()> {
        self.push_work_internal(&item)
            .with_context(|| format!("for request: {:?}", item))
    }

    fn push_work_internal(&mut self, item: &TaskRequest) -> Result<()> {
        self.targets.push_slice(&item.model.targets)?;
        self.store.request_all(item);
        Ok(())
    }

    pub async fn run(mut self) -> Result<Vec<TaskResponse>> {
        let mut response_rx = self.caller.request_responses();
        self.targets.flush()?;
        let yarrp_task = tokio::task::spawn_blocking(move || self.caller.consume_run(self.targets));
        while let Some(response) = response_rx.recv().await {
            trace!("response from yarrp: {:?}", response);
            self.store.register_response(response);
        }
        response_rx.close(); // ensure nothing else is sent
        yarrp_task
            .await
            .with_context(|| "during blocking yarrp call (await)")??;
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
            }),
            None => TraceResult::NoResponse {
                target_addr: value.addr,
            },
        }
    }
}
