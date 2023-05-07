use ipnet::Ipv6Net;

use queue_models::echo_response::EchoProbeResponse;

use crate::prefix_split::SubnetSample;
use crate::zmap_call::ProbeResponse;

use super::ProbeStore;
use super::dispatch::ProbeStoreDispatcher;
use super::model::RoutableProbeStore;
use super::subnet::SubnetStore;

#[derive(Debug)]
pub struct PrefixStoreDispatcher {
    prefix: Ipv6Net,
    dispatcher: ProbeStoreDispatcher<SubnetStore>,
}

impl PrefixStoreDispatcher {
    fn new(prefix: Ipv6Net, samples: Vec<SubnetSample>) -> Self {
        let dispatcher = ProbeStoreDispatcher::new_prefilled(samples);
        Self { prefix, dispatcher }
    }
}

impl ProbeStoreDispatcher<PrefixStoreDispatcher> {
    pub fn register_request(&mut self, prefix: Ipv6Net, samples: Vec<SubnetSample>) {
        let prefix_store = PrefixStoreDispatcher::new(prefix, samples);
        self.stores.push(prefix_store);
    }
}

impl Into<EchoProbeResponse> for PrefixStoreDispatcher {
    fn into(self) -> EchoProbeResponse {
        EchoProbeResponse {
            target_net: self.prefix,
            splits: self.dispatcher.stores.into_iter()
                .map(|it| it.into())
                .collect(),
        }
    }
}

impl RoutableProbeStore for PrefixStoreDispatcher {
    fn is_responsible_for(&self, probe: &ProbeResponse) -> bool {
        self.dispatcher.is_responsible_for(probe)
    }
}

impl ProbeStore for PrefixStoreDispatcher {
    fn register_response(&mut self, response: &ProbeResponse) {
        self.dispatcher.register_response(response)
    }

    fn fill_missing(&mut self) {
        self.dispatcher.fill_missing()
    }
}
