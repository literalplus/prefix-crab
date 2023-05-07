use std::fmt::Debug;

use crate::prefix_split::SubnetSample;
use crate::zmap_call::ProbeResponse;

mod model;
mod subnet;
mod dispatch;
#[cfg(test)]
mod test_utils;

pub trait ProbeStore {
    /// Registers a response with the store.
    fn register_response(&mut self, response: &ProbeResponse);

    /// Marks any response slots for which no response has been registered yet with
    /// as not having received a response.
    fn fill_missing(&mut self);
}

pub fn create_for(samples: Vec<SubnetSample>) -> impl ProbeStore + Debug {
    dispatch::ProbeStoreDispatcher::new(samples)
}
