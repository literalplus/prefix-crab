pub use queue_models::echo_response::{DestUnreachKind, ResponseCount, ResponseKey};

use crate::schedule::ProbeResponse;

impl From<&ProbeResponse> for ResponseKey {
    fn from(source: &ProbeResponse) -> Self {
        match source.icmp_type {
            1 => Self::DestinationUnreachable {
                kind: DestUnreachKind::parse(source.icmp_code),
            },
            3 => Self::TimeExceeded { from: source.source_ip, sent_ttl: source.original_ttl },
            129 => Self::EchoReply {
                different_from: Some(source.source_ip)
                    .filter(|it| *it != source.original_dest_ip),
                sent_ttl: source.original_ttl,
            },
            _ => Self::Other { description: source.classification.to_string() }
        }
    }
}

pub trait RoutableProbeStore {
    fn is_responsible_for(&self, probe: &ProbeResponse) -> bool;
}
