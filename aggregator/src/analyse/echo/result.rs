use std::fmt::Display;
use std::net::Ipv6Addr;
use std::ops::{Deref, DerefMut};

use crate::analyse::Interpretation;

#[derive(Debug, Default)]
pub struct EchoResult {
    parent: Interpretation,
    pub follow_ups: Vec<EchoFollowUp>,
}

impl Deref for EchoResult {
    type Target = Interpretation;

    fn deref(&self) -> &Self::Target {
        &self.parent
    }
}

impl DerefMut for EchoResult {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.parent
    }
}


impl EchoResult {
    pub fn needs_follow_up(&self) -> bool {
        !self.follow_ups.is_empty()
    }
}

impl Display for EchoResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EchoResult")
            .field("parent", &format!("{}", self.parent))
            .field("follow_ups", &format!("{} entries", self.follow_ups.len()))
            .finish()
    }
}

#[derive(Debug)]
pub struct EchoFollowUp {
    pub targets: Vec<Ipv6Addr>,
    pub for_responsive: bool,
}
