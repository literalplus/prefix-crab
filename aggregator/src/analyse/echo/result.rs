use std::net::Ipv6Addr;

use prefix_crab::prefix_split::NetIndex;

use crate::analyse::{LastHopRouterSource, FollowUp};

pub use super::EchoResult;

#[derive(Debug)]
pub struct EchoSplitResult {
    pub net_index: NetIndex,
    pub follow_ups: Vec<FollowUp>,
    pub last_hop_routers: Vec<LastHopRouter>,
    pub weird_behaviours: Vec<WeirdBehaviour>,
}

impl EchoSplitResult {
    pub fn new(net_index: NetIndex) -> EchoSplitResult {
        Self {
            net_index,
            follow_ups: vec![],
            last_hop_routers: vec![],
            weird_behaviours: vec![],
        }
    }
}

#[derive(Debug)]
pub struct LastHopRouter {
    pub address: Ipv6Addr,
    pub source: LastHopRouterSource,
    pub hit_count: u16,
}

#[derive(Debug)]
pub enum WeirdBehaviour {
    TtlExceeded { sent_ttl: u8 },
    OtherWeird { description: String },
}

impl WeirdBehaviour {
    pub fn get_id(&self) -> String {
        match &self {
            Self::TtlExceeded { sent_ttl } => {
                format!("ttl-xc-{}", sent_ttl)
            }
            Self::OtherWeird { description } => {
                format!("o-{}", description)
            }
        }
    }
}
