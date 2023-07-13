use std::net::Ipv6Addr;

use ipnet::Ipv6Net;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct EchoProbeResponse {
    pub target_net: Ipv6Net,
    pub splits: Vec<SplitResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SplitResult {
    pub net: Ipv6Net,
    /// This is a list s.t. it can be easily represented as JSON, but every key should
    /// only occur once. The recommended behaviour is to ignore any repetitions of a key, but
    /// undefined behaviour is also allowed if more convenient.
    pub responses: Vec<Responses>,
}

#[derive(Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub enum ResponseKey {
    DestinationUnreachable { kind: DestUnreachKind, from: Ipv6Addr },
    EchoReply { different_from: Option<Ipv6Addr>, sent_ttl: u8 },
    NoResponse,
    TimeExceeded { from: Ipv6Addr, sent_ttl: u8 },
    Other { description: String },
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