

use derive_where::derive_where;

use queue_models::probe_response::EchoProbeResponse;

use crate::schedule::ProbeResponse;
use crate::zmap_call::SENT_TTL;
use prefix_crab::prefix_split::{PrefixSplit, SubnetSample};

use super::dispatch::ProbeStoreDispatcher;
use super::model::RoutableProbeStore;
use super::subnet::SubnetStore;
use super::ProbeStore;

#[derive_where(Debug; ExtraData: core::fmt::Debug)]
pub struct PrefixStoreDispatcher<ExtraData: Sized> {
    split: PrefixSplit,
    dispatcher: ProbeStoreDispatcher<SubnetStore>,
    pub extra_data: ExtraData,
}

impl<ExtraData: Sized> PrefixStoreDispatcher<ExtraData> {
    fn new(split: PrefixSplit, samples: Vec<SubnetSample>, extra_data: ExtraData) -> Self {
        let dispatcher = ProbeStoreDispatcher::new_prefilled(samples);
        Self {
            split,
            dispatcher,
            extra_data,
        }
    }
}

impl<ExtraData: Sized> ProbeStoreDispatcher<PrefixStoreDispatcher<ExtraData>> {
    pub fn register_request(
        &mut self,
        split: PrefixSplit,
        samples: Vec<SubnetSample>,
        extra_data: ExtraData,
    ) {
        let prefix_store = PrefixStoreDispatcher::new(split, samples, extra_data);
        self.stores.push(prefix_store);
    }
}

impl<ExtraData: Sized> From<PrefixStoreDispatcher<ExtraData>> for EchoProbeResponse {
    fn from(val: PrefixStoreDispatcher<ExtraData>) -> Self {
        let PrefixSplit {
            base_net: target_net,
            subnet_prefix_len,
            ..
        } = val.split;
        EchoProbeResponse {
            target_net,
            subnet_prefix_len,
            sent_ttl: SENT_TTL,
            splits: val
                .dispatcher
                .stores
                .into_iter()
                .map(|it| it.into())
                .collect(),
        }
    }
}

impl<ExtraData: Sized> RoutableProbeStore for PrefixStoreDispatcher<ExtraData> {
    fn is_responsible_for(&self, probe: &ProbeResponse) -> bool {
        self.dispatcher.is_responsible_for(probe)
    }
}

impl<ExtraData: Sized> ProbeStore for PrefixStoreDispatcher<ExtraData> {
    fn register_response(&mut self, response: &ProbeResponse) {
        self.dispatcher.register_response(response)
    }

    fn fill_missing(&mut self) {
        self.dispatcher.fill_missing()
    }
}
