use std::net::Ipv6Addr;
use std::{collections::HashMap, fmt::Display};

use crate::analyse::{map64, WeirdType};
use crate::analyse::{map64::Net64Map, HitCount, LastHopRouter, LhrSource, WeirdNode};

#[derive(Debug, Default)]
pub struct EchoResult {
    store: Net64Map<PrefixEntry>,
    pub follow_ups: Vec<EchoFollowUp>,
}

impl EchoResult {
    pub fn register_lhrs(&mut self, targets: &[Ipv6Addr], lhr: LhrAddr, source: LhrSource) {
        for target in targets.iter() {
            self.store[target].register_lhr(lhr, source);
        }
    }

    pub fn register_weirds(
        &mut self,
        targets: &[Ipv6Addr],
        description: WeirdType,
    ) {
        if targets.len() == 1 { // no need to clone if there is only one item (the usual case)
            let only_item = targets.iter().next().expect("one target to be present if length is one");
            self.store[only_item].register_weird(description);
            return;
        }
        for target in targets.iter() {
            self.store[target].register_weird(description.clone());
        }
    }

    pub fn count_other_responsive(&mut self, targets: &[Ipv6Addr]) {
        for target in targets.iter() {
            self.store[target].responsive_count += 1;
        }
    }

    pub fn count_unresponsive(&mut self, targets: &[Ipv6Addr]) {
        for target in targets.iter() {
            self.store[target].unresponsive_count += 1;
        }
    }

    pub fn iter(&self) -> map64::IterEntries<PrefixEntry> {
        self.store.iter_entries()
    }

    pub fn drain(&mut self) -> map64::Drain<PrefixEntry> {
        self.store.drain()
    }
}

impl Display for EchoResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EchoResult")
            .field("store", &format!("{} entries", self.store.len()))
            .field("follow_ups", &format!("{} entries", self.follow_ups.len()))
            .finish()
    }
}

pub type LhrAddr = Ipv6Addr;
pub type WeirdAddr = Ipv6Addr;

#[derive(Debug, Default)]
pub struct PrefixEntry {
    pub last_hop_routers: HashMap<LhrAddr, LastHopRouter>,
    pub weird: HashMap<WeirdType, WeirdNode>,
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

    pub fn register_weird(&mut self, description: WeirdType) {
        self.weird
            .entry(description)
            .or_default()
            .register();
        self.responsive_count += 1;
    }
}

#[derive(Debug)]
pub enum EchoFollowUp {
    TraceResponsive { targets: Vec<Ipv6Addr> },
    TraceUnresponsive { targets: Vec<Ipv6Addr> },
}
