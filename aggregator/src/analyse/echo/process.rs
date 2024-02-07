use std::net::Ipv6Addr;

use db_model::analyse::WeirdType;
use tracing::instrument;

use super::result::*;
use log::{debug, warn};
use queue_models::probe_response::{
    EchoProbeResponse,
    ResponseKey::{self, *},
    Responses, SplitResult,
};

#[instrument(name = "analyse echo", skip_all)]
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
        DestinationUnreachable { kind, from } => match kind.try_into() {
            Ok(source) => result.register_lhrs(targets, *from, source),
            Err(id) => {
                let typ = match id {
                    5 => WeirdType::DestUnreachFailedEgress,
                    6 => WeirdType::DestUnreachRejectRoute,
                    it => {
                        debug!("Unknown dest-unreach kind: {:?}", it);
                        WeirdType::DestUnreachOther
                    }
                };
                result.register_weirds(targets, typ);
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
            warn!("Received unexpected ICMP type: {}", description);
            result.register_weirds(targets, WeirdType::UnexpectedIcmpType);
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
