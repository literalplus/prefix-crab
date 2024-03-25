pub mod forest;
pub mod map64;
pub mod subnet;
mod tree;
mod analysis;

/// percent rating
/// at least 0, not more than 255
/// 100 means that we are sufficiently confident for some action to be taken
pub type Confidence = u8;

pub const CONFIDENCE_THRESH: Confidence = 100;

pub type LhrAddr = Ipv6Addr;

use std::net::Ipv6Addr;

pub use tree::*;
pub use analysis::*;