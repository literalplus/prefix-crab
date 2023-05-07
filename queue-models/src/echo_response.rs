use std::collections::HashMap;
use std::net::Ipv6Addr;

use ipnet::Ipv6Net;

#[derive(Debug)]
pub struct EchoProbeResponse {
    pub target_net: Ipv6Net,
    pub splits: Vec<SplitResult>,
}

#[derive(Debug)]
pub struct SplitResult {
    pub net: Ipv6Net,
    pub responses: HashMap<ResponseKey, Responses>,
}

#[derive(Eq, PartialEq, Hash, Debug)]
pub enum ResponseKey {
    DestinationUnreachable { kind: DestUnreachKind },
    EchoReply { different_from: Option<Ipv6Addr> },
    NoResponse,
    TimeExceeded { from: Ipv6Addr, sent_ttl: u8 },
    Other { description: String },
}

pub type ResponseCount = usize;

#[derive(Debug)]
pub struct Responses {
    pub intended_targets: Vec<Ipv6Addr>,
}

impl Responses {
    pub fn len(&self) -> ResponseCount {
        self.intended_targets.len()
    }
}


#[derive(Eq, PartialEq, Hash, Debug)]
pub enum DestUnreachKind {
    /// 0 = no route, 2 = beyond scope, 7 = source routing error
    Other(u8),
    AdminProhibited,
    AddressUnreachable,
    PortUnreachable,
    // FailedEgressPolicy, // ZMAPv6 doesn't seem to currently capture this...
    // RejectRoute,        // ^ same
}

impl DestUnreachKind {
    pub fn parse(code: u8) -> Self {
        match code {
            1 => Self::AdminProhibited,
            3 => Self::AddressUnreachable,
            4 => Self::PortUnreachable,
            // 5 => Self::FailedEgressPolicy,
            // 6 => Self::RejectRoute,
            weird => Self::Other(weird),
        }
    }
}