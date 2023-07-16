use std::fmt::Debug;
use derive_where::derive_where;

use log::warn;

use prefix_crab::prefix_split::SubnetSample;
use crate::probe_store::model::RoutableProbeStore;
use crate::probe_store::ProbeStore;
use crate::schedule::ProbeResponse;

use super::subnet::SubnetStore;

#[derive_where(Debug; T: Debug)]
pub struct ProbeStoreDispatcher<T> where T: RoutableProbeStore + ProbeStore {
    pub stores: Vec<T>,
}

impl<T> ProbeStoreDispatcher<T> where T: RoutableProbeStore + ProbeStore {
    pub fn new() -> Self {
        Self { stores: vec![] }
    }
}

impl ProbeStoreDispatcher<SubnetStore> {
    pub fn new_prefilled(samples: Vec<SubnetSample>) -> Self {
        let stores = samples.into_iter()
            .map(SubnetStore::new)
            .collect();
        Self { stores }
    }
}

impl<T> ProbeStore for ProbeStoreDispatcher<T> where T: RoutableProbeStore + ProbeStore + Debug {
    fn register_response(&mut self, response: &ProbeResponse) {
        let mut already_found = false;
        for store in self.stores.iter_mut() {
            if store.is_responsible_for(response) {
                if already_found {
                    warn!(
                        "A probe response {:?} was handled by more than one subnet. \
                        This shouldn't be a huge issue in practice, but we should be \
                        aware that it happened.", response
                    );
                }
                // NOTE: A single response might be registered with multiple subnets,
                // as there is no way to tell which sample it belongs to if the samples overlap
                // This seems like the most gentle way without introducing additional constraints
                // e.g. delay overlapping samples...
                store.register_response(response);
                already_found = true
            }
        }
    }

    fn fill_missing(&mut self) {
        for store in self.stores.iter_mut() {
            store.fill_missing();
        }
    }
}

impl<T> RoutableProbeStore for ProbeStoreDispatcher<T> where T: RoutableProbeStore + ProbeStore + Debug {
    fn is_responsible_for(&self, probe: &ProbeResponse) -> bool {
        for store in self.stores.iter() {
            if store.is_responsible_for(probe) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use anyhow::*;

    use super::*;
    use super::super::test_utils::*;

    #[test]
    fn register_single_match() -> Result<()> {
        // given
        let sample_a = gen_sample("2001:db8:cafe::/48")?;
        let addr_a = sample_a.addresses[0];
        let sample_b = gen_sample("2001:db8:beef::/48")?;
        let mut dispatcher = ProbeStoreDispatcher::new_prefilled(
            vec![sample_a, sample_b]
        );
        // when
        dispatcher.register_response(&gen_timxceed(addr_a));
        // then
        then_timxceed_registered(&mut dispatcher.stores[0], addr_a);
        then_no_timxceed_registered(&mut dispatcher.stores[1]);
        Ok(())
    }

    #[test]
    fn register_multi_match() -> Result<()> {
        /*
         We evaluate only the _intended_ target, so this should not mis-assign cases where
         adjacent subnets are related. if two samples request overlapping subnets, there's not
         really a way to figure out which of these the probe was intended for without first doing
         an overlap check and implementing a - potentially complicated and error-prone - delay
         of infringing subnets. We expect downstream to handle these cases appropriately, and
         in general, this shouldn't occur too often.
         */

        // given
        let sample_a = gen_sample("2001:db8::/32")?;
        let addr_a = sample_a.addresses[0];
        let sample_b = gen_sample("2001:db8::/32")?;
        let addr_b = sample_b.addresses[0];
        assert_ne!(addr_a, addr_b); // bad luck
        let mut dispatcher = ProbeStoreDispatcher::new_prefilled(
            vec![sample_a, sample_b]
        );
        // when
        dispatcher.register_response(&gen_timxceed(addr_a));
        dispatcher.register_response(&gen_timxceed(addr_b));
        // then
        then_timxceed_registered(&mut dispatcher.stores[0], addr_a);
        then_timxceed_registered(&mut dispatcher.stores[1], addr_a);
        then_timxceed_registered(&mut dispatcher.stores[0], addr_b);
        then_timxceed_registered(&mut dispatcher.stores[1], addr_b);
        Ok(())
    }
}