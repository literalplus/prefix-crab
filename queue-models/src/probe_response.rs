use std::net::Ipv6Addr;

use ipnet::Ipv6Net;
use serde::{Deserialize, Serialize};

use crate::{
    probe_request::{EchoProbeRequest, TraceRequest, TraceRequestId},
    RoutedMessage, TypeRoutedMessage,
};

#[derive(Debug, Serialize, Deserialize)]
pub enum ProbeResponse {
    Echo(EchoProbeResponse),
    Trace(TraceResponse),
}

impl RoutedMessage for ProbeResponse {
    fn routing_key(&self) -> &'static str {
        match self {
            ProbeResponse::Echo(msg) => msg.routing_key(),
            ProbeResponse::Trace(msg) => msg.routing_key(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EchoProbeResponse {
    pub target_net: Ipv6Net,
    pub subnet_prefix_len: u8,
    pub sent_ttl: u8,
    pub splits: Vec<SplitResult>,
}

impl TypeRoutedMessage for EchoProbeResponse {
    fn routing_key() -> &'static str {
        <EchoProbeRequest as TypeRoutedMessage>::routing_key()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SplitResult {
    pub net_index: u8,
    /// This is a list s.t. it can be easily represented as JSON, but every key should
    /// only occur once. The recommended behaviour is to ignore any repetitions of a key, but
    /// undefined behaviour is also allowed if more convenient.
    pub responses: Vec<Responses>,
}

#[derive(Eq, PartialEq, Hash, Debug, Serialize, Deserialize, Clone)]
pub enum ResponseKey {
    DestinationUnreachable {
        kind: DestUnreachKind,
        from: Ipv6Addr,
    },
    EchoReply {
        different_from: Option<Ipv6Addr>,
    },
    NoResponse,
    TimeExceeded {
        from: Ipv6Addr,
    },
    Other {
        from: Ipv6Addr,
        description: String,
    },
}

impl ResponseKey {
    pub fn get_dest_unreach_kind(&self) -> Option<&DestUnreachKind> {
        match self {
            Self::DestinationUnreachable { kind, from: _ } => Some(kind),
            _ => None,
        }
    }
}

pub type ResponseCount = usize;

#[derive(Debug, Serialize, Deserialize)]
pub struct Responses {
    pub key: ResponseKey,
    pub intended_targets: Vec<Ipv6Addr>,
}

impl Responses {
    pub fn len(&self) -> ResponseCount {
        self.intended_targets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Eq, PartialEq, Hash, Debug, Serialize, Deserialize, Copy, Clone)]
pub enum DestUnreachKind {
    /// 2 = beyond scope, 7 = source routing error
    Other(u8),
    NoRoute,
    AdminProhibited,
    AddressUnreachable,
    PortUnreachable,
    // FailedEgressPolicy, // ZMAPv6 doesn't seem to currently capture this...
    // RejectRoute,        // ^ same
}

impl DestUnreachKind {
    pub fn parse(code: u8) -> Self {
        match code {
            0 => Self::NoRoute,
            1 => Self::AdminProhibited,
            3 => Self::AddressUnreachable,
            4 => Self::PortUnreachable,
            // 5 => Self::FailedEgressPolicy,
            // 6 => Self::RejectRoute,
            weird => Self::Other(weird),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TraceResponse {
    pub id: TraceRequestId,
    pub results: Vec<TraceResult>,
}

impl TypeRoutedMessage for TraceResponse {
    fn routing_key() -> &'static str {
        <TraceRequest as TypeRoutedMessage>::routing_key()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TraceResult {
    LastResponsiveHop(LastHop),
    NoResponse { target_addr: Ipv6Addr }, // unlikely; usually we'd expect at least some transit router to respond
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LastHop {
    pub target_addr: Ipv6Addr,
    pub last_hop_addr: Ipv6Addr,
    /// Sent TTL that yielded this last hop. Note that we are collecting the last *responsive* hop,
    /// and if target_ttl is set, this need not be exactly target_ttl - 1.
    pub last_hop_ttl: u8,
    /// Sent TTL that yielded the actual target, if a response was received from it.
    pub target_ttl: Option<u8>,
}
