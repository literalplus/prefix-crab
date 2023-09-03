use std::{collections::HashMap, net::Ipv6Addr};

use log::warn;
use queue_models::probe_request::{TraceRequest, TraceRequestId};

use crate::schedule::ProbeResponse;

#[derive(Debug, Default)]
pub struct ProbeStore {
    store: HashMap<Ipv6Addr, Entry>,
}

impl ProbeStore {
    pub fn request_all(&mut self, request: TraceRequest) {
        for target_addr in request.targets {
            if self
                .store
                .insert(target_addr, Entry::new(request.id))
                .is_some()
            {
                warn!(
                    "Single target {} was requested by multiple concurrent requests, \
                    this should be highly unlikely due to randomised target addresses.",
                    target_addr,
                );
            }
        }
    }

    pub fn register_response(&mut self, response: ProbeResponse) {
        let key = if response.intended_target.is_unspecified() {
            response.actual_from
        } else {
            response.intended_target
        };
        let mut entry = self.store.get_mut(&key);
        if entry.is_none() {
            warn!("Received response for an unknown target {}, ignoring.", key);
            return;
        }
        entry.unwrap().register_response(response);
    }
}

#[derive(Debug)]
struct Entry {
    request_id: TraceRequestId,
    responses: HashMap<u8, ProbeResponse>,
}

impl Entry {
    fn new(request_id: TraceRequestId) -> Self {
        Self {
            request_id,
            responses: HashMap::new(),
        }
    }

    fn register_response(&mut self, response: ProbeResponse) {
        if self.responses.insert(response.sent_ttl, response).is_some() {
            warn!(
                "Received another response for TTL {} of one of the addresses in {}, overwriting.",
                response.sent_ttl, self.request_id
            );
        }
    }
}
