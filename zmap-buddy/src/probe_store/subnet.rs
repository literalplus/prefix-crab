use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::collections::hash_map::Iter;
use std::net::Ipv6Addr;
use std::ops::Index;

use queue_models::echo_response;
use queue_models::echo_response::SplitResult;

use prefix_crab::prefix_split::SubnetSample;
use crate::probe_store::model::RoutableProbeStore;
use crate::probe_store::ProbeStore;
use crate::schedule::ProbeResponse;

use super::model::ResponseKey;

/// Stores aggregate information about responses.
/// It is implied that this is somehow keyed, but it can also be used without that.
#[derive(Debug)]
pub struct Responses {
    pub intended_targets: Vec<Ipv6Addr>,
}

impl Responses {
    fn empty() -> Self {
        return Responses {
            intended_targets: vec![],
        };
    }

    fn add(&mut self, source: &ProbeResponse) {
        self.intended_targets.push(source.original_dest_ip);
    }

    fn add_missed(&mut self, addr: Ipv6Addr) {
        self.intended_targets.push(addr);
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.intended_targets.len()
    }

    fn to_model(self, key: ResponseKey) -> echo_response::Responses {
        echo_response::Responses {
            key,
            intended_targets: self.intended_targets,
        }
    }
}


/// Stores responses for a specific subnet sample, keyed by the type of response.
#[derive(Debug)]
pub struct SubnetStore {
    // NOTE: Addresses that receive a response are REMOVED from the sample
    sample: SubnetSample,
    responses: HashMap<ResponseKey, Responses>,
}

impl SubnetStore {
    pub fn new(sample: SubnetSample) -> Self {
        Self {
            sample,
            responses: HashMap::new(),
        }
    }

    #[cfg(test)]
    pub fn is_waiting_for_response(&self, addr: Ipv6Addr) -> bool {
        self.sample.addresses.contains(&addr)
    }

    fn entry(&mut self, key: ResponseKey) -> &mut Responses {
        self.responses.entry(key).or_insert(Responses::empty())
    }

    #[cfg(test)]
    pub fn iter(&self) -> Iter<'_, ResponseKey, Responses> {
        self.responses.iter()
    }
}

impl RoutableProbeStore for SubnetStore {
    fn is_responsible_for(&self, response: &ProbeResponse) -> bool {
        self.sample.subnet.contains(&response.original_dest_ip)
    }
}

impl Index<ResponseKey> for SubnetStore {
    type Output = Responses;

    fn index(&self, index: ResponseKey) -> &Self::Output {
        self.responses.index(&index)
    }
}

impl ProbeStore for SubnetStore {
    fn register_response(&mut self, response: &ProbeResponse) {
        let key = ResponseKey::from(response);
        let aggregate = self.entry(key);
        aggregate.add(&response);
        // Using a HashSet here is unlikely to provide a good trade-off, as there
        // will usually only be 16 elements (potentially duplicated for small subnets)
        self.sample.addresses.retain(|el| *el != response.original_dest_ip);
    }

    fn fill_missing(&mut self) {
        if self.sample.addresses.is_empty() {
            return;
        }
        let missing_addrs_iter = self.sample.addresses.drain(..);
        let missing_addrs_uniq = HashSet::<_>::from_iter(missing_addrs_iter);
        let no_responses = self.entry(ResponseKey::NoResponse);
        for missing_addr in missing_addrs_uniq {
            no_responses.add_missed(missing_addr);
        }
    }
}

impl Into<SplitResult> for SubnetStore {
    fn into(self) -> SplitResult {
        SplitResult {
            net: self.sample.subnet,
            responses: self.responses.into_iter()
                .map(|(k, v)| v.to_model(k))
                .collect(),
        }
    }
}


#[cfg(test)]
mod tests {
    use anyhow::*;
    use assertor::*;

    use super::*;
    use super::super::test_utils::*;

    #[test]
    fn registered_response_is_retained() -> Result<()> {
        // given
        let prefix = gen_any_sample()?;
        let ip_addr = prefix.addresses[0].clone();
        let mut store = SubnetStore::new(prefix);
        // when
        store.register_response(&gen_timxceed(ip_addr));
        // then
        then_timxceed_registered(&mut store, ip_addr);
        Ok(())
    }

    #[test]
    fn fill_missing_fills() -> Result<()> {
        // given
        let prefix = gen_any_sample()?;
        let target_addrs = prefix.addresses.clone();
        let mut store = SubnetStore::new(prefix);
        // when
        store.fill_missing();
        // then
        let no_res = &store.responses[&ResponseKey::NoResponse];
        assert_that!(no_res.intended_targets).contains_exactly(target_addrs);
        assert_eq!(no_res.len(), 16);
        Ok(())
    }

    #[test]
    fn fill_missing_ignores_existing() -> Result<()> {
        // given
        let prefix = gen_any_sample()?;
        let responsive_addr = prefix.addresses[0].clone();
        let mut store = SubnetStore::new(prefix);
        store.register_response(&gen_timxceed(responsive_addr));
        // when
        store.fill_missing();
        // then
        let no_res = &store.responses[&ResponseKey::NoResponse];
        assert_eq!(no_res.len(), 15);
        then_timxceed_registered(&mut store, responsive_addr);
        Ok(())
    }

    #[test]
    fn is_respo_checks_intended_not_actual_pos() -> Result<()> {
        // given
        let prefix = gen_sample("2001:db8:cafe::/48")?;
        let mut res = gen_timxceed("2001:db8:cafe::1".parse::<Ipv6Addr>()?);
        res.source_ip = "2001:db8:beef::1".parse::<Ipv6Addr>()?;
        let store = SubnetStore::new(prefix);
        // when
        let is_respo = store.is_responsible_for(&res);
        let is_waiting = store.is_waiting_for_response(res.source_ip);
        // then
        assert!(is_respo);
        assert!(!is_waiting); // bad luck - this means that the code could also
        // evaluate whether it's waiting for that address, which is undesired
        Ok(())
    }

    #[test]
    fn is_respo_checks_intended_not_actual_neg() -> Result<()> {
        // given
        let prefix = gen_sample("2001:db8:cafe::/48")?;
        let mut res = gen_timxceed("2001:db8:beef::1".parse::<Ipv6Addr>()?);
        res.source_ip = "2001:db8:cafe::1".parse::<Ipv6Addr>()?;
        let store = SubnetStore::new(prefix);
        // when
        let is_respo = store.is_responsible_for(&res);
        // then
        assert!(!is_respo);
        Ok(())
    }
}
