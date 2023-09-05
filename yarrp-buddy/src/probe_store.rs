use std::{collections::HashMap, net::Ipv6Addr};

use itertools::Itertools;
use log::warn;
use queue_models::probe_request::TraceRequestId;

use crate::schedule::{ProbeResponse, TaskRequest};

#[derive(Debug, Default)]
pub struct ProbeStore {
    store: HashMap<Ipv6Addr, Target>,
    acks_per_request: HashMap<u128, u64>,
}

impl ProbeStore {
    pub fn request_all(&mut self, req: &TaskRequest) {
        self.acks_per_request
            .insert(req.model.id.uuid().as_u128(), req.delivery_tag_to_ack);
        for target_addr in req.model.targets.iter() {
            if self
                .store
                .insert(*target_addr, Target::new(*target_addr, req.model.id))
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
        let entry = self.store.get_mut(&key);
        if entry.is_none() {
            warn!("Received response for an unknown target {}, ignoring.", key);
            return;
        }
        entry.unwrap().register_response(response);
    }
}

#[derive(Debug)]
pub struct Target {
    pub addr: Ipv6Addr,
    pub request_id: TraceRequestId,
    pub last_hop: Option<Hop>,
    pub target_own_ttl: Option<u8>,
}

impl Target {
    fn new(addr: Ipv6Addr, request_id: TraceRequestId) -> Self {
        Self {
            addr,
            request_id,
            last_hop: None,
            target_own_ttl: None,
        }
    }

    fn register_response(&mut self, response: ProbeResponse) {
        if response.actual_from == self.addr {
            if let Some(old_ttl) = self.target_own_ttl {
                warn!("Target was hit twice, old TTL is {} and new response {:?}", old_ttl, response);
            }
            self.target_own_ttl = Some(response.sent_ttl);
        } else if self.is_better_last_hop(&response) {
            self.last_hop = Some(Hop { addr: response.actual_from, sent_ttl: response.sent_ttl });
        }
    }

    fn is_better_last_hop(&self, response: &ProbeResponse) -> bool {
        // assume that it's not for the target itself, that is checked in register_response()
        if response.icmp_type != 3 /* time exceeded */ {
            warn!("Got a traceroute response other than time exceeded: {:?}", response);
            return false
        }
        match &self.last_hop {
            None => true,
            Some(existing) => response.sent_ttl > existing.sent_ttl,
        }
    }
}

#[derive(Debug)]
pub struct Hop {
    pub addr: Ipv6Addr,
    pub sent_ttl: u8,
}

#[derive(Debug)]
pub struct RequestGroup {
    pub request_id: TraceRequestId,
    pub targets: Vec<Target>,
    pub delivery_tag: u64,
}

impl RequestGroup {
    fn new(request_id: TraceRequestId, delivery_tag: u64) -> Self {
        Self {
            request_id,
            targets: vec![],
            delivery_tag,
        }
    }

    fn add(&mut self, entry: Target) {
        self.targets.push(entry);
    }
}

impl ProbeStore {
    pub fn into_request_groups(self) -> Vec<RequestGroup> {
        let group_by = self
            .store
            .into_values()
            .sorted_by_key(|e| e.request_id)
            .group_by(|e| e.request_id);

        let mut results = vec![];
        for (id, entries) in group_by.into_iter() {
            let delivery_tag = self
                .acks_per_request
                .get(&id.uuid().as_u128())
                .expect("request to be in ack store");
            let mut group = RequestGroup::new(id, *delivery_tag);
            for entry in entries {
                group.add(entry)
            }
            results.push(group)
        }
        results
    }
}
