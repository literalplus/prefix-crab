use std::collections::HashSet;
use std::net::Ipv6Addr;
use std::{collections::HashMap, fmt::Display};

use db_model::analyse::map64::{self, Net64Map};

use crate::analyse::WeirdType;
use crate::analyse::{HitCount, LhrSource};

pub type LhrAddr = Ipv6Addr;

/// Interpretation of a probe response
#[derive(Debug, Default)]
pub struct Interpretation {
    store: Net64Map<Prefix>,
}

impl Interpretation {
    pub fn register_lhrs(&mut self, targets: &[Ipv6Addr], lhr: LhrAddr, source: LhrSource) {
        for target in targets.iter() {
            self.store[target].register_lhr(lhr, source);
        }
    }

    pub fn register_weirds(&mut self, targets: &[Ipv6Addr], description: WeirdType) {
        if targets.len() == 1 {
            // no need to clone if there is only one item (the usual case)
            let only_item = targets
                .iter()
                .next()
                .expect("one target to be present if length is one");
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

    pub fn drain(&mut self) -> map64::Drain<Prefix> {
        self.store.drain()
    }
}

impl Display for Interpretation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Interpretation with {} prefixes affected", self.store.len())
    }
}

#[derive(Debug, Default)]
pub struct Prefix {
    pub last_hop_routers: HashMap<LhrAddr, LastHopRouter>,
    pub weird: HashMap<WeirdType, WeirdNode>,
    /// Probes that resulted in any sort of response
    pub responsive_count: HitCount,
    /// Probes that stayed dark
    pub unresponsive_count: HitCount,
}

impl Prefix {
    pub fn register_lhr(&mut self, lhr: LhrAddr, source: LhrSource) {
        self.last_hop_routers
            .entry(lhr)
            .or_default()
            .register(source);
        self.responsive_count += 1;
    }

    pub fn register_weird(&mut self, description: WeirdType) {
        self.weird.entry(description).or_default().register();
        self.responsive_count += 1;
    }
}

#[derive(Debug, Default)]
pub struct LastHopRouter {
    pub sources: HashSet<LhrSource>,
    pub hit_count: HitCount,
}

impl LastHopRouter {
    pub fn register(&mut self, source: LhrSource) {
        self.sources.insert(source);
        self.hit_count += 1;
    }
}

#[derive(Debug, Default)]
pub struct WeirdNode {
    pub hit_count: HitCount,
}

impl WeirdNode {
    pub fn register(&mut self) {
        self.hit_count += 1;
    }
}
