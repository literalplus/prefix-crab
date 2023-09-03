pub use queue_models::probe_response::{DestUnreachKind, ResponseCount, ResponseKey};

use crate::schedule::ProbeResponse;

impl From<&ProbeResponse> for ResponseKey {
    fn from(source: &ProbeResponse) -> Self {
        match source.icmp_type {
            1 => Self::DestinationUnreachable {
                kind: DestUnreachKind::parse(source.icmp_code),
                from: source.source_ip,
            },
            3 => Self::TimeExceeded { from: source.source_ip },
            129 => Self::EchoReply {
                different_from: Some(source.source_ip)
                    .filter(|it| *it != source.original_dest_ip),
            },
            _ => Self::Other { from: source.source_ip, description: source.classification.to_string() }
        }
    }
}

pub trait RoutableProbeStore {
    fn is_responsible_for(&self, probe: &ProbeResponse) -> bool;
}
