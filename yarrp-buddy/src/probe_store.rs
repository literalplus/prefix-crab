use std::{collections::HashMap, net::Ipv6Addr};

use anyhow::bail;
use itertools::Itertools;
use log::{warn, info};
use queue_models::{
    probe_request::TraceRequestId,
    probe_response::{DestUnreachKind, TraceResponseType},
};

use crate::schedule::{ProbeResponse, TaskRequest};

#[derive(Debug, Default)]
pub struct ProbeStore {
    store: HashMap<Ipv6Addr, Target>,
    empty_requests: Vec<TraceRequestId>,
    acks_per_request: HashMap<u128, u64>,
}

impl ProbeStore {
    pub fn request_all(&mut self, req: &TaskRequest) {
        self.acks_per_request
            .insert(req.model.id.uuid().as_u128(), req.delivery_tag_to_ack);
        if req.model.targets.is_empty() {
            self.empty_requests.push(req.model.id);
            return;
        }
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
        let key = if response.intended_target.is_unspecified() { // intended may be :: if we don't know
            response.actual_from // <-- likely not super useful, as this is the router, but better than ::
        } else {
            response.intended_target
        };
        let entry = self.store.get_mut(&key);
        if entry.is_none() {
            // most likely this is an echo reply coming from the router directly, and it wasn't caught in the
            // ZMAP stage due to ICMP rate limiting or similar. Echo replies don't quote the incoming packet header,
            // so the intended target cannot be determined (?).
            info!(
                "Received response for an unknown target {} - directed at {} and coming from {}, ignoring.",
                 key, response.intended_target, response.actual_from
                );
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
                warn!(
                    "Target was hit twice, old TTL is {} and new response {:?}",
                    old_ttl, response
                );
            }
            self.target_own_ttl = Some(response.sent_ttl);
        }
        let hop = match Hop::try_from(&response) {
            Ok(hop) => hop,
            Err(e) => {
                warn!("Failed to construct hop: {:?}", e);
                return;
            }
        };
        if self.is_better_last_hop(&hop) {
            self.last_hop = Some(hop);
        }
    }

    fn is_better_last_hop(&self, hop: &Hop) -> bool {
        match &self.last_hop {
            None => true,
            Some(existing) => hop.sent_ttl > existing.sent_ttl,
        }
    }
}

#[derive(Debug)]
pub struct Hop {
    pub addr: Ipv6Addr,
    pub sent_ttl: u8,
    pub response_type: TraceResponseType,
}

impl TryFrom<&ProbeResponse> for Hop {
    type Error = anyhow::Error;

    fn try_from(value: &ProbeResponse) -> Result<Self, Self::Error> {
        use TraceResponseType as T;

        let response_type = match value.icmp_type {
            1 => T::DestinationUnreachable {
                kind: DestUnreachKind::parse(value.icmp_code),
            },
            3 => T::TimeExceeded,
            129 => T::EchoReply,
            _ => bail!(
                "Received unexpected ICMP type {} from yarrp",
                value.icmp_type
            ),
        };
        Ok(Hop {
            addr: value.actual_from,
            sent_ttl: value.sent_ttl,
            response_type,
        })
    }
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
            let mut group = Self::make_group(&self.acks_per_request, id);
            for entry in entries {
                group.add(entry)
            }
            results.push(group)
        }
        for id in self.empty_requests {
            // Upstream clears some less-promising requests to reduce overall load
            results.push(Self::make_group(&self.acks_per_request, id));
        }
        results
    }

    fn make_group(acks_per_request: &HashMap<u128, u64>, id: TraceRequestId) -> RequestGroup {
        let delivery_tag = acks_per_request
            .get(&id.uuid().as_u128())
            .expect("request to be in ack store");
        RequestGroup::new(id, *delivery_tag)
    }
}
