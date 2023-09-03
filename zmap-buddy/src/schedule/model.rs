use std::net::Ipv6Addr;
use queue_models::probe_response::EchoProbeResponse;
use queue_models::probe_request::EchoProbeRequest;

#[derive(Debug, serde::Deserialize)]
pub struct ProbeResponse {
    /*
This currently encodes some zmap-specific details (field names). Could be updated
to map from a zmap-specific struct. The business module needs to own the interface
struct though.
 */

    #[serde(rename = "type")]
    pub icmp_type: u8,
    #[serde(rename = "code")]
    pub icmp_code: u8,
    pub original_ttl: u8,
    #[serde(rename = "orig-dest-ip")] // unknown why only this field is kebab-case
    pub original_dest_ip: Ipv6Addr,
    #[serde(rename = "saddr")]
    pub source_ip: Ipv6Addr,
    pub classification: String,
}

#[derive(Debug)]
pub struct TaskRequest {
    pub model: EchoProbeRequest,

    /// Which delivery tag needs to be acknowledged after the request is complete.
    /// This is used so that only requests that were actually fully processed are
    /// removed from the queue (e.g. buddy crashes).
    pub delivery_tag_to_ack: u64,
}

#[derive(Debug)]
pub struct TaskResponse {
    pub model: EchoProbeResponse,

    /// The delivery tag that this response handles, and which shall thus be ack'd.
    pub acks_delivery_tag: u64,
}
