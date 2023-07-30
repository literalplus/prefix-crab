use std::collections::HashMap;
use std::net::Ipv6Addr;

use crate::analyse::{
    map64::Net64Map,
    HitCount, LastHopRouter, WeirdNode,
    LhrSource::{self, *},
};

#[derive(Debug, Default)]
pub struct EchoResult {
    store: Net64Map<PrefixEntry>,
    pub follow_ups: Vec<EchoFollowUp>,
}

impl EchoResult {
    pub fn register_lhrs(&mut self, target_addrs: &Vec<Ipv6Addr>, lhr: LhrAddr, source: LhrSource) {
        for target in target_addrs.iter() {
            self.store[target].register_lhr(lhr, source);
        }
    }

    pub fn register_weirds(&mut self, target_addrs: &Vec<Ipv6Addr>, addr: WeirdAddr, description: &str) {
        for target in target_addrs.iter() {
            self.store[target].register_weird(addr, description)
        }
    }

    pub fn count_other_responsive(&mut self, target_addrs: &Vec<Ipv6Addr>) {
        for target in target_addrs.iter() {
            self.store[target].responsive_count += 1;
        }
    }

    pub fn count_unresponsive(&mut self, target_addrs: &Vec<Ipv6Addr>) {
        for target in target_addrs.iter() {
            self.store[target].unresponsive_count += 1;
        }
    }
}

pub type LhrAddr = Ipv6Addr;
pub type WeirdAddr = Ipv6Addr;

#[derive(Debug, Default)]
pub struct PrefixEntry {
    pub last_hop_routers: HashMap<LhrAddr, LastHopRouter>,
    pub weird_nodes: HashMap<WeirdAddr, WeirdNode>,
    /// Probes that results in any response
    pub responsive_count: HitCount,
    /// Probes that did not result in a response
    pub unresponsive_count: HitCount,
}

impl PrefixEntry {
    pub fn register_lhr(&mut self, lhr: LhrAddr, source: LhrSource) {
        self.last_hop_routers
            .entry(lhr)
            .or_default()
            .register(source);
        self.responsive_count += 1;
    }

    pub fn register_weird(&mut self, addr: WeirdAddr, description: &str) {
        self.weird_nodes
            .entry(addr)
            .or_default()
            .register(description);
        self.responsive_count += 1;
    }
}

#[derive(Debug)]
pub enum EchoFollowUp {
    TraceResponsive { targets: Vec<Ipv6Addr> },
    TraceUnresponsive { targets: Vec<Ipv6Addr> },
}
