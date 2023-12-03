pub mod forest;
mod tree;
pub mod map64;

/// percent rating, i.e. between 0 and 100
pub type Confidence = u8;

pub const MAX_CONFIDENCE: Confidence = 100;

pub use tree::*;
