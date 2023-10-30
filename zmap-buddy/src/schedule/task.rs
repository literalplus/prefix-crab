use anyhow::{Context, Result};
use log::trace;

use crate::probe_store::{self, PrefixSplitProbeStore, PrefixStoreDispatcher, ProbeStore};
use crate::zmap_call::{self, Caller, TargetCollector};
use prefix_crab::prefix_split::*;

use super::interleave::InterleavedTargetsIter;
use super::{TaskRequest, TaskResponse};

pub struct SchedulerTask<'req> {
    store: PrefixSplitProbeStore<&'req TaskRequest>,
    caller: Caller,
    targets: TargetCollector,
}

impl<'req> SchedulerTask<'req> {
    pub fn new(zmap_params: zmap_call::Params) -> Result<Self> {
        Ok(Self {
            store: probe_store::create(),
            caller: zmap_params.to_caller_assuming_sudo()?,
            targets: TargetCollector::new_default()?,
        })
    }

    pub fn push_work(&mut self, item: &'req TaskRequest) -> Result<()> {
        self.push_work_internal(item)
            .with_context(|| format!("for request: {:?}", item))
    }

    fn push_work_internal(&mut self, item: &'req TaskRequest) -> Result<()> {
        // NOTE: Since the targets are randomly chosen, we don't need to additionally permute them.
        //       We interleave the different subnets to reduce load on a single subnet, in the hopes that
        //       at least a few of the subnets in a batch would belong to a different router. This should also
        //       help somewhat reduce ICMP rate limiting.

        let base_net = item.model.target_net;
        let split = split(base_net).context("splitting IPv6 prefix")?;
        let samples = split.to_samples(super::SAMPLES_PER_SUBNET);
        for addr in InterleavedTargetsIter::new(&samples) {
            self.targets
                .push(&addr)
                .context("pushing targets")?;
        }
        self.store.register_request(split, samples, item);
        Ok(())
    }

    pub async fn run(mut self) -> Result<Vec<TaskResponse>> {
        let mut response_rx = self.caller.request_responses();
        self.targets.flush()?;
        let zmap_task = tokio::task::spawn_blocking(move || {
            trace!("Now calling zmap");
            self.caller.consume_run(self.targets)
        });
        let mut not_moved_store = self.store;
        while let Some(record) = response_rx.recv().await {
            trace!("response from zmap: {:?}", record);
            not_moved_store.register_response(&record);
        }
        response_rx.close(); // ensure nothing else is sent
        zmap_task
            .await
            .with_context(|| "during blocking zmap call (await)")??;
        not_moved_store.fill_missing();
        Ok(map_into_responses(not_moved_store))
    }
}

fn map_into_responses(store: PrefixSplitProbeStore<&TaskRequest>) -> Vec<TaskResponse> {
    store.stores.into_iter().map(map_into_response).collect()
}

fn map_into_response(store: PrefixStoreDispatcher<&TaskRequest>) -> TaskResponse {
    let acks_delivery_tag = store.extra_data.delivery_tag_to_ack;
    TaskResponse {
        model: store.into(),
        acks_delivery_tag,
    }
}
