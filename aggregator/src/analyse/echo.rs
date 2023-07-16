use anyhow::Result;
use log::warn;
use prefix_crab::prefix_split::NetIndex;
use queue_models::echo_response::{
    DestUnreachKind::{self, *},
    EchoProbeResponse,
    ResponseKey::{self, *},
    SplitResult,
};
use result::{WeirdBehaviour::*, *};

use super::{
    FollowUp::{TraceResponsive, TraceUnresponsive},
    LastHopRouterSource::{self, *},
};

pub mod result;
mod store;

#[derive(Debug)]
pub struct EchoResult {
    pub splits: Vec<EchoSplitResult>,
}

pub fn process(model: &EchoProbeResponse) -> EchoResult {
    let mut splits = vec![];
    for split in &model.splits {
        if let Result::Ok(valid_index) = split.net_index.try_into() {
            splits.push(process_split(split, valid_index));
        } else {
            warn!(
                "Ignoring result[{}] due to net index out of range",
                split.net_index
            );
        }
    }
    EchoResult { splits }
}

fn process_split(split: &SplitResult, index: NetIndex) -> EchoSplitResult {
    let mut result = EchoSplitResult::new(index);
    let mut unresponsive_addrs = vec![];
    for response in &split.responses {
        match &response.key {
            DestinationUnreachable { kind, from } => {
                if let Some(source) = kind_to_source(kind) {
                    result.last_hop_routers.push(LastHopRouter {
                        address: *from,
                        source,
                        hit_count: u16::try_from(response.intended_targets.len())
                            .unwrap_or(u16::MAX),
                    })
                }
            }
            EchoReply {
                different_from: _,
                sent_ttl,
            } => {
                // TODO handle different from somehow ?
                result.follow_ups.push(TraceResponsive {
                    targets: response.intended_targets.clone(),
                    sent_ttl: *sent_ttl,
                })
            }
            ResponseKey::Other { description } => result.weird_behaviours.push(OtherWeird {
                description: description.to_string(),
            }),
            NoResponse => unresponsive_addrs.extend(&response.intended_targets),
            TimeExceeded { from: _, sent_ttl } => result.weird_behaviours.push(TtlExceeded {
                sent_ttl: *sent_ttl,
            }),
        }
    }
    let nothing_else_recorded = result.follow_ups.is_empty() && result.last_hop_routers.is_empty();
    if !unresponsive_addrs.is_empty() && nothing_else_recorded {
        result.follow_ups.push(TraceUnresponsive {
            candidates: unresponsive_addrs,
        })
    }
    result
}

fn kind_to_source(value: &DestUnreachKind) -> Option<LastHopRouterSource> {
    Some(match value {
        NoRoute => DestUnreachReject,
        AdminProhibited => DestUnreachProhibit,
        AddressUnreachable => DestUnreachAddrPort,
        PortUnreachable => DestUnreachAddrPort,
        DestUnreachKind::Other(_) => return None,
    })
}
