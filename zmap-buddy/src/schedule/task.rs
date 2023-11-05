use anyhow::{Context, Result};
use log::{info, trace};
use prefix_crab::blocklist::{self, PrefixBlocklist};

use crate::probe_store::{self, PrefixSplitProbeStore, PrefixStoreDispatcher, ProbeStore};
use crate::zmap_call::{Caller, TargetCollector};
use prefix_crab::prefix_split::*;

use super::interleave::InterleavedTargetsIter;
use super::{TaskRequest, TaskResponse};

pub struct SchedulerTask<'req> {
    store: PrefixSplitProbeStore<&'req TaskRequest>,
    caller: Caller,
    target_samples: Vec<SubnetSample>,
    blocklist: PrefixBlocklist,
}

impl<'req> SchedulerTask<'req> {
    pub fn new(params: super::Params) -> Result<Self> {
        Ok(Self {
            store: probe_store::create(),
            caller: params.base.to_caller_assuming_sudo()?,
            target_samples: vec![],
            blocklist: blocklist::read(params.blocklist)?,
        })
    }

    pub fn push_work(&mut self, item: &'req TaskRequest) -> Result<()> {
        self.push_work_internal(item)
            .with_context(|| format!("for request: {:?}", item))
    }

    fn push_work_internal(&mut self, item: &'req TaskRequest) -> Result<()> {
        let base_net = item.model.target_net;
        let split = split(base_net).context("splitting IPv6 prefix")?;

        // Stage samples instead of pushing directly to allow interweaving of different requests
        let samples = if self.blocklist.is_whole_net_blocked(&base_net) {
            // TODO signal blockage of whole net in response?
            vec![]
        } else {
            split.to_samples(super::SAMPLES_PER_SUBNET)
        };
        self.target_samples.extend_from_slice(&samples);

        self.store.register_request(split, samples, item);
        Ok(())
    }

    pub async fn run(mut self) -> Result<Vec<TaskResponse>> {
        let mut response_rx = self.caller.request_responses();
        let targets = self.collect_targets()?;
        let zmap_task = tokio::task::spawn_blocking(move || {
            trace!("Now calling zmap");
            self.caller.consume_run(targets)
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

    fn collect_targets(&mut self) -> Result<TargetCollector> {
        // NOTE: Since the targets are randomly chosen, we don't need to additionally permute them.
        //       We interleave the different subnets to reduce load on a single subnet, in the hopes that
        //       at least a few of the subnets in a batch would belong to a different router. This should also
        //       help somewhat reduce ICMP rate limiting.

        let mut targets = TargetCollector::new_default()?;
        for addr in InterleavedTargetsIter::new(&self.target_samples) {
            if self.blocklist.is_blocked(&addr) {
                info!("Omitting {} due to blocklist", addr);
            } else {
                targets.push(&addr).context("pushing targets")?;
            }
        }
        self.target_samples = vec![];
        targets.flush()?;
        Ok(targets)
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
