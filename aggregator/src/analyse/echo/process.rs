use std::net::Ipv6Addr;

use crate::analyse::WeirdType;

use super::result::*;
use log::debug;
use queue_models::probe_response::{
    DestUnreachKind::{self, *},
    EchoProbeResponse,
    ResponseKey::{self, *},
    Responses, SplitResult,
};

use super::super::LhrSource::{self, *};

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
        DestinationUnreachable { kind, from } => match kind_to_source(kind) {
            Ok(source) => result.register_lhrs(targets, *from, source),
            Err(id) => {
                result.register_weirds(
                    targets,
                    WeirdType::DestUnreachableUnexpectedKind { kind: id },
                );
                debug!("Unknown dest-unreach kind: {:?}", id);
            }
        },
        EchoReply {
            different_from: None,
        } => {
            follow_up_collector.stage_responsive(targets);
            result.count_other_responsive(targets);
        }
        EchoReply {
            different_from: Some(_),
        } => {
            follow_up_collector.stage_responsive(targets);
            result.register_weirds(targets, WeirdType::DifferentEchoReplySource);
        }
        ResponseKey::Other {
            from: _,
            description,
        } => {
            result.register_weirds(
                targets,
                WeirdType::UnexpectedIcmpType {
                    description: description.to_string(),
                },
            );
        }
        NoResponse => {
            follow_up_collector.stage_unresponsive(targets);
            result.count_unresponsive(targets);
        }
        TimeExceeded { from: _ } => result.register_weirds(targets, WeirdType::TtlExceededForEcho),
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
        if value.targets.is_empty() {
            None
        } else {
            Some(EchoFollowUp {
                targets: value.targets,
                for_responsive: value.has_responsive,
            })
        }
    }
}

fn kind_to_source(value: &DestUnreachKind) -> Result<LhrSource, u8> {
    Ok(match value {
        NoRoute => UnreachRoute,
        AdminProhibited => UnreachAdmin,
        AddressUnreachable => UnreachAddr,
        PortUnreachable => UnreachPort,
        DestUnreachKind::Other(kind) => return Err(*kind),
    })
}
