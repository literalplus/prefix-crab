pub mod forest;
mod tree;
pub mod map64;
pub mod subnet;

/// percent rating, i.e. between 0 and 100
pub type Confidence = u8;

pub const MAX_CONFIDENCE: Confidence = 100;

pub type LhrAddr = Ipv6Addr;

use std::net::Ipv6Addr;

pub use tree::*;
