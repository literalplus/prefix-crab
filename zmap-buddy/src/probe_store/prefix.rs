use derive_where::derive_where;
use ipnet::Ipv6Net;

use queue_models::echo_response::EchoProbeResponse;

use prefix_crab::prefix_split::SubnetSample;
use crate::schedule::ProbeResponse;

use super::dispatch::ProbeStoreDispatcher;
use super::model::RoutableProbeStore;
use super::ProbeStore;
use super::subnet::SubnetStore;

#[derive_where(Debug; ExtraData: core::fmt::Debug)]
pub struct PrefixStoreDispatcher<ExtraData: Sized> {
    prefix: Ipv6Net,
    dispatcher: ProbeStoreDispatcher<SubnetStore>,
    pub extra_data: ExtraData,
}

impl<ExtraData: Sized> PrefixStoreDispatcher<ExtraData> {
    fn new(prefix: Ipv6Net, samples: Vec<SubnetSample>, extra_data: ExtraData) -> Self {
        let dispatcher = ProbeStoreDispatcher::new_prefilled(samples);
        Self { prefix, dispatcher, extra_data }
    }
}

impl<ExtraData: Sized> ProbeStoreDispatcher<PrefixStoreDispatcher<ExtraData>> {
    pub fn register_request(
        &mut self, prefix: Ipv6Net, samples: Vec<SubnetSample>, extra_data: ExtraData,
    ) {
        let prefix_store = PrefixStoreDispatcher::new(
            prefix, samples, extra_data,
        );
        self.stores.push(prefix_store);
    }
}

impl<ExtraData: Sized> Into<EchoProbeResponse> for PrefixStoreDispatcher<ExtraData> {
    fn into(self) -> EchoProbeResponse {
        EchoProbeResponse {
            target_net: self.prefix,
            splits: self.dispatcher.stores.into_iter()
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
