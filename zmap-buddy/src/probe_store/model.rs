use std::net::Ipv6Addr;

use crate::zmap_call::ProbeResponse;

#[derive(Eq, PartialEq, Hash, Debug)]
pub enum DestUnreachKind {
    /// 0 = no route, 2 = beyond scope, 7 = source routing error
    Other(u8),
    AdminProhibited,
    AddressUnreachable,
    PortUnreachable,
    FailedEgressPolicy,
    RejectRoute,
}

impl DestUnreachKind {
    pub fn parse(code: u8) -> Self {
        match code {
            1 => Self::AdminProhibited,
            3 => Self::AddressUnreachable,
            4 => Self::PortUnreachable,
            5 => Self::FailedEgressPolicy, // ZMAPv6 doesn't seem to currently capture this...
            6 => Self::RejectRoute,        // ^ same
            weird => Self::Other(weird),
        }
    }
}

#[derive(Eq, PartialEq, Hash, Debug)]
pub enum ResponseKey {
    DestinationUnreachable { kind: DestUnreachKind },
    EchoReply { different_from: Option<Ipv6Addr> },
    NoResponse,
    TimeExceeded { from: Ipv6Addr, sent_ttl: u8 },
    Other { description: String },
}

impl ResponseKey {
    pub fn from(source: &ProbeResponse) -> Self {
        match source.icmp_type {
            1 => Self::DestinationUnreachable {
                kind: DestUnreachKind::parse(source.icmp_code),
            },
            3 => Self::TimeExceeded { from: source.source_ip, sent_ttl: source.original_ttl },
            129 => Self::EchoReply {
                different_from: Some(source.source_ip)
                    .filter(|it| *it != source.original_dest_ip),
            },
            _ => Self::Other { description: source.classification.to_string() }
        }
    }
}

pub type ResponseCount = u8;
