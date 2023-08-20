use std::collections::HashSet;

use crate::analyse::{HitCount, LhrSource};

pub use super::echo::result::*;
use super::WeirdType;

pub trait CanFollowUp {
    fn needs_follow_up(&self) -> bool;
}

impl CanFollowUp for EchoResult {
    fn needs_follow_up(&self) -> bool {
        !self.follow_ups.is_empty()
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
