pub use prefix::PrefixStoreDispatcher;

use crate::schedule::ProbeResponse;

mod model;
mod subnet;
mod prefix;
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

pub type PrefixSplitProbeStore<ExtraData> = dispatch::ProbeStoreDispatcher<
    PrefixStoreDispatcher<ExtraData>
>;

pub fn create<ExtraData: Sized>() -> PrefixSplitProbeStore<ExtraData> {
    dispatch::ProbeStoreDispatcher::new()
}
