use std::collections::HashMap;

use anyhow::{Context, Result};
use log::trace;
use queue_models::probe_request::TraceRequestId;

use crate::{yarrp_call::{self, Caller, TargetCollector}, probe_store::ProbeStore};

use super::{TaskRequest, TaskResponse};

pub struct SchedulerTask {
    store: ProbeStore,
    acks_per_request: HashMap<u128, u8>,
    caller: Caller,
    targets: TargetCollector,
}

impl SchedulerTask {
    pub fn new(zmap_params: yarrp_call::Params) -> Result<Self> {
        Ok(Self {
            store: ProbeStore::default(),
            acks_per_request: HashMap::new(),
            caller: zmap_params.to_caller_assuming_sudo()?,
            targets: TargetCollector::new_default()?,
        })
    }

    pub fn push_work(&mut self, item: TaskRequest) -> Result<()> {
        // FIXME hasher
        self.acks_per_request.insert(item.model.id.into(), item.delivery_tag_to_ack);
        self.push_work_internal(item).with_context(|| format!("for request: {:?}", item))
    }

    fn push_work_internal(&mut self, item: TaskRequest) -> Result<()> {
        self.targets.push_slice(&item.model.targets)?;
        self.store.request_all(item.model);
        Ok(())
    }

    pub async fn run(mut self) -> Result<Vec<TaskResponse>> {
        let mut response_rx = self.caller.request_responses();
        self.targets.flush()?;
        let yarrp_task = tokio::task::spawn_blocking(move || {
            self.caller.consume_run(self.targets)
        });
        while let Some(response) = response_rx.recv().await {
            trace!("response from yarrp: {:?}", response);
            self.store.register_response(response);
        }
        response_rx.close(); // ensure nothing else is sent
        yarrp_task.await.with_context(|| "during blocking yarrp call (await)")??;
        Ok(map_into_responses(self.store))
    }
}

fn map_into_responses(store: ProbeStore) -> Vec<TaskResponse> {
    store.stores.into_iter()
        .map(map_into_response)
        .collect()
}

fn map_into_response(store: PrefixStoreDispatcher<&TaskRequest>) -> TaskResponse {
    let acks_delivery_tag = store.extra_data.delivery_tag_to_ack;
    TaskResponse { model: store.into(), acks_delivery_tag }
}
