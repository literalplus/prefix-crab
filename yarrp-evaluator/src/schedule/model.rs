use std::net::Ipv6Addr;
use ipnet::Ipv6Net;
use queue_models::probe_response::{TraceResponse, TraceResult};

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
pub struct EvaluateRequest {
    pub net: Ipv6Net,
    pub targets: Vec<Ipv6Addr>,
}

#[derive(Debug)]
pub struct EvaluateResponse {
    pub net: Ipv6Net,
    pub results: Vec<TraceResult>,
}