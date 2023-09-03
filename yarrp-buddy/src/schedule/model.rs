use std::net::Ipv6Addr;
use queue_models::probe_response::TraceResponse;
use queue_models::probe_request::TraceRequest;

#[derive(Debug, serde::Deserialize)]
pub struct ProbeResponse {
    pub icmp_type: u8,
    pub icmp_code: u8,
    pub sent_ttl: u8,
    pub intended_target: Ipv6Addr,
    pub actual_from: Ipv6Addr,
    pub received_ttl: u8,
}

#[derive(Debug)]
pub struct TaskRequest {
    pub model: TraceRequest,

    /// Which delivery tag needs to be acknowledged after the request is complete.
    /// This is used so that only requests that were actually fully processed are
    /// removed from the queue (e.g. buddy crashes).
    pub delivery_tag_to_ack: u64,
}

#[derive(Debug)]
pub struct TaskResponse {
    pub model: TraceResponse,

    /// The delivery tag that this response handles, and which shall thus be ack'd.
    pub acks_delivery_tag: u64,
}
