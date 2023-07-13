

use std::net::Ipv6Addr;


use ipnet::Ipv6Net;


use queue_models::echo_response::{DestUnreachKind, EchoProbeResponse, SplitResult};
use queue_models::echo_response::ResponseKey::*;

enum FollowUpTraceRequest {
    TraceResponsive { targets: Vec<Ipv6Addr>, sent_ttl: u8 },
    TraceUnresponsive { candidates: Vec<Ipv6Addr> },
}

enum LastHopRouterSource {
    TraceUnresponsive,
    TraceResponsive,
    DestinationUnreachable { kind: DestUnreachKind },
}

struct LastHopRouter {
    router: Ipv6Addr,
    handled_addresses: Vec<Ipv6Addr>,
    source: LastHopRouterSource,
}

enum WeirdBehaviourKind {
    TtlExceeded { sent_ttl: u8 },
    Other { description: String },
}

struct WeirdBehaviour {
    from: Ipv6Addr,
    kind: WeirdBehaviourKind,
}

struct SplitAnalysisResult {
    split: Ipv6Net,
    follow_ups: Vec<FollowUpTraceRequest>,
    last_hop_routers: Vec<LastHopRouter>,
    weird_behaviours: Vec<WeirdBehaviour>,
}

impl SplitAnalysisResult {
    fn new(split: &SplitResult) -> SplitAnalysisResult {
        Self {
            split: split.net,
            follow_ups: vec![],
            last_hop_routers: vec![],
            weird_behaviours: vec![],
        }
    }
}

pub fn process(model: &EchoProbeResponse) {
    let mut split_results = vec![];
    for split in &model.splits {
        split_results.push(process_split(split));
    }
    // TODO handle results of split processing
}

fn process_split(split: &SplitResult) -> SplitAnalysisResult {
    let mut result = SplitAnalysisResult::new(split);
    let mut some_addrs_were_unresponsive = false;
    for response in &split.responses {
        match &response.key {
            DestinationUnreachable { kind, from } => {
                result.last_hop_routers.push(LastHopRouter {
                    router: *from,
                    handled_addresses: response.intended_targets.clone(),
                    source: LastHopRouterSource::DestinationUnreachable { kind: *kind },
                })
            }
            EchoReply { different_from: _, sent_ttl } => {
                result.follow_ups.push(FollowUpTraceRequest::TraceResponsive {
                    targets: response.intended_targets.clone(),
                    sent_ttl: *sent_ttl,
                })
            }
            Other { description } => {
                for target in &response.intended_targets {
                    result.weird_behaviours.push(WeirdBehaviour {
                        from: *target,
                        kind: WeirdBehaviourKind::Other {
                            description: description.to_string(),
                        },
                    })
                }
            }
            NoResponse => {
                some_addrs_were_unresponsive = true;
            }
            TimeExceeded { from, sent_ttl } => {
                result.weird_behaviours.push(WeirdBehaviour {
                    from: *from,
                    kind: WeirdBehaviourKind::TtlExceeded { sent_ttl: *sent_ttl },
                })
            }
        }
    }
    let nothing_else_recorded = result.follow_ups.is_empty() &&
        result.last_hop_routers.is_empty();
    if some_addrs_were_unresponsive && nothing_else_recorded {
        // TODO
    }
    result
}
