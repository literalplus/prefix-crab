use db_model::{analyse::Confidence, prefix_tree::{PriorityClass, PrefixTree}};
use ipnet::Ipv6Net;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct LeafNet {
    pub net: Ipv6Net,
    pub priority_class: PriorityClass,
    pub confidence: Confidence,
}

impl From<PrefixTree> for LeafNet {
    fn from(value: PrefixTree) -> Self {
        Self {
            net: value.net,
            priority_class: value.priority_class,
            confidence: value.confidence,
        }
    }
}

// Separate error struct needed to implement Clone (and this is also the reason for the weird desc thing)
#[derive(Debug, Error, Clone)]
pub enum Error {
    #[error("Connecting to DB: {desc}")]
    DbConnect { desc: String },
    #[error("Loading tree: {desc}")]
    LoadTree { desc: String },
}

pub(super) type StdResult<T, E> = std::result::Result<T, E>;
pub type Result = StdResult<Vec<LeafNet>, Error>;
