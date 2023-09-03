use std::net::Ipv6Addr;

use ipnet::Ipv6Net;
use serde::{Deserialize, Serialize};
use type_safe_id::{StaticType, TypeSafeId};

use crate::{RoutedMessage, TypeRoutedMessage};

#[derive(Debug, Serialize, Deserialize)]
pub enum ProbeRequest {
    /// Scan a prefix using ICMP Echo Requests by splitting it into sub-prefixes
    /// and selecting random addresses to probe.
    Echo(EchoProbeRequest),
    /// Trace a set of addresses in a prefix to determine the Last-Hop-Router.
    Trace(TraceRequest),
}

impl RoutedMessage for ProbeRequest {
    fn routing_key(&self) -> &'static str {
        match self {
            ProbeRequest::Echo(msg) => msg.routing_key(),
            ProbeRequest::Trace(msg) => msg.routing_key(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EchoProbeRequest {
    pub target_net: Ipv6Net,
}

impl TypeRoutedMessage for EchoProbeRequest {
    fn routing_key() -> &'static str {
        "echo"
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TraceRequest {
    pub id: TraceRequestId,
    pub targets: Vec<Ipv6Addr>,
    /// Whether the requested targets were responsive or not. This may be used to prioritise requests or otherwise
    /// optimise the implementation of the probe sender.
    pub were_responsive: bool,
}

impl TypeRoutedMessage for TraceRequest {
    fn routing_key() -> &'static str {
        "trace"
    }
}

// Use a marker type, otherwise TraceRequest can't refer to its own ID (recursive type)
#[derive(Default)]
pub struct TraceIdTypeMarker;

impl StaticType for TraceIdTypeMarker {
    const TYPE: &'static str = "tracerq";
}

pub type TraceRequestId = TypeSafeId<TraceIdTypeMarker>;
