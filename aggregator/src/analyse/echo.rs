use std::net::Ipv6Addr;

use log::debug;
use queue_models::echo_response::{
    DestUnreachKind::{self, *},
    EchoProbeResponse,
    ResponseKey::{self, *},
    Responses, SplitResult,
};
use result::*;

use super::LhrSource::{self, *};

mod persist;
pub mod result;

pub fn process(model: &EchoProbeResponse) -> EchoResult {
    let mut result = EchoResult::default();
    for split in &model.splits {
        process_split(&mut result, split);
    }
    result
}

fn process_split(result: &mut EchoResult, split: &SplitResult) {
    let mut follow_up_collector = FollowUpCollector::new();
    for responses in &split.responses {
        process_responses(result, responses, &mut follow_up_collector);
    }
    if let Some(follow_up) = follow_up_collector.into() {
        result.follow_ups.push(follow_up);
    }
}

fn process_responses(
    result: &mut EchoResult,
    responses: &Responses,
    follow_up_collector: &mut FollowUpCollector,
) {
    let targets = &responses.intended_targets;
    match &responses.key {
        DestinationUnreachable { kind, from } => {
            if let Some(source) = kind_to_source(kind) {
                result.register_lhrs(targets, *from, source);
            } else {
                debug!(
                    "Unknown dest-unreach kind: {:?} -- IGNORING this response.",
                    kind
                );
            }
        }
        EchoReply {
            different_from: None,
        } => {
            follow_up_collector.stage_responsive(targets);
            result.count_other_responsive(targets);
        }
        EchoReply {
            different_from: Some(from),
        } => {
            follow_up_collector.stage_responsive(targets);
            result.register_weirds(targets, *from, "echo-reply-diff-src");
        }
        ResponseKey::Other { from, description } => {
            result.register_weirds(targets, *from, description);
        }
        NoResponse => {
            follow_up_collector.stage_unresponsive(targets);
            result.count_unresponsive(targets);
        }
        TimeExceeded { from } => result.register_weirds(targets, *from, "ttlx"),
    }
}

/// Records target addresses for a follow-up trace request, preferring responsive targets.
/// Once a responsive target is recorded, all unresponsive targets are discarded.
struct FollowUpCollector {
    targets: Vec<Ipv6Addr>,
    has_responsive: bool,
}

impl FollowUpCollector {
    fn new() -> Self {
        Self {
            targets: vec![],
            has_responsive: false,
        }
    }

    fn stage_responsive(&mut self, targets: &Vec<Ipv6Addr>) {
        if !self.has_responsive {
            self.targets = targets.clone();
        } else {
            self.targets.extend(targets);
        }
    }

    fn stage_unresponsive(&mut self, targets: &Vec<Ipv6Addr>) {
        if !self.has_responsive {
            self.targets.extend(targets);
        }
    }
}

impl From<FollowUpCollector> for Option<EchoFollowUp> {
    fn from(value: FollowUpCollector) -> Self {
        match value {
            it if it.targets.is_empty() => None,
            FollowUpCollector {
                targets,
                has_responsive,
            } if has_responsive => Some(EchoFollowUp::TraceResponsive { targets }),
            it => Some(EchoFollowUp::TraceResponsive {
                targets: it.targets,
            }),
        }
    }
}

fn kind_to_source(value: &DestUnreachKind) -> Option<LhrSource> {
    Some(match value {
        NoRoute => DestUnreachReject,
        AdminProhibited => DestUnreachProhibit,
        AddressUnreachable => DestUnreachAddrPort,
        PortUnreachable => DestUnreachAddrPort,
        DestUnreachKind::Other(_) => return None,
    })
}
