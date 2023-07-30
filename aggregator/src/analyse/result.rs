use std::collections::HashSet;

use crate::analyse::{LhrSource, HitCount};

pub use super::echo::result::*;

pub trait CanFollowUp {
    fn needs_follow_up(&self) -> bool;
}

impl CanFollowUp for EchoResult {
    fn needs_follow_up(&self) -> bool {
        return !self.follow_ups.is_empty();
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
    pub descriptions: HashSet<String>,
    pub hit_count: HitCount,
}

impl WeirdNode {
    pub fn register(&mut self, description: &str) {
        if !self.descriptions.contains(description) {
            self.descriptions.insert(description.to_string());
        }
        self.hit_count += 1;
    }
}
