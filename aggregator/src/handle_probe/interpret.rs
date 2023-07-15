use anyhow::*;

use queue_models::echo_response::EchoProbeResponse;

use self::model::EchoResult;

pub mod model {
    pub use super::echo::{
        EchoSplitResult, FollowUpTraceRequest, LastHopRouter, LastHopRouterSource,
    };

    #[derive(Debug)]
    pub struct EchoResult {
        pub splits: Vec<EchoSplitResult>,
    }

    pub trait CanFollowUp {
        fn needs_follow_up(&self) -> bool;
    }

    impl CanFollowUp for EchoResult {
        fn needs_follow_up(&self) -> bool {
            return self.splits.iter().any(|it| it.needs_follow_up());
        }
    }
}

pub fn process_echo(model: &EchoProbeResponse) -> Result<EchoResult> {
    let mut splits = vec![];
    for split in &model.splits {
        splits.push(echo::process_split(split));
    }
    return Ok(EchoResult { splits });
}

mod echo {
    
    use std::net::Ipv6Addr;

    use ipnet::Ipv6Net;

    use queue_models::echo_response::ResponseKey::*;
    use queue_models::echo_response::{DestUnreachKind, SplitResult};
    use FollowUpTraceRequest::*;
    use WeirdBehaviour::*;

    use super::model::CanFollowUp;

    #[derive(Debug)]
    pub struct EchoSplitResult {
        pub split: Ipv6Net,
        pub follow_ups: Vec<FollowUpTraceRequest>,
        pub last_hop_routers: Vec<LastHopRouter>,
        pub weird_behaviours: Vec<WeirdBehaviour>,
    }

    impl CanFollowUp for EchoSplitResult {
        fn needs_follow_up(&self) -> bool {
            return !self.follow_ups.is_empty();
        }
    }

    // TODO type should probably be moved into a follow_up module
    #[derive(Debug)]
    pub enum FollowUpTraceRequest {
        TraceResponsive {
            targets: Vec<Ipv6Addr>,
            sent_ttl: u8,
        },
        TraceUnresponsive {
            candidates: Vec<Ipv6Addr>,
        },
    }

    #[derive(Debug)]
    pub enum LastHopRouterSource {
        TraceUnresponsive,
        TraceResponsive,
        DestinationUnreachable { kind: DestUnreachKind },
    }

    #[derive(Debug)]
    pub struct LastHopRouter {
        pub router: Ipv6Addr,
        handled_addresses: Vec<Ipv6Addr>,
        pub source: LastHopRouterSource,
    }

    impl LastHopRouter {
        pub fn get_hit_count(&self) -> usize {
            self.handled_addresses.len()
        }
    }

    #[derive(Debug)]
    pub enum WeirdBehaviour {
        TtlExceeded { sent_ttl: u8 },
        OtherWeird { description: String },
    }

    impl WeirdBehaviour {
        pub fn get_id(&self) -> String {
            match &self {
                Self::TtlExceeded { sent_ttl } => {
                    format!("ttl-xc-{}", sent_ttl)
                }
                Self::OtherWeird { description } => {
                    format!("o-{}", description)
                }
            }
        }
    }

    impl EchoSplitResult {
        fn new(split: &SplitResult) -> EchoSplitResult {
            Self {
                split: split.net,
                follow_ups: vec![],
                last_hop_routers: vec![],
                weird_behaviours: vec![],
            }
        }
    }

    pub fn process_split(split: &SplitResult) -> EchoSplitResult {
        let mut result = EchoSplitResult::new(split);
        let mut unresponsive_addrs = vec![];
        for response in &split.responses {
            match &response.key {
                DestinationUnreachable { kind, from } => {
                    result.last_hop_routers.push(LastHopRouter {
                        router: *from,
                        handled_addresses: response.intended_targets.clone(),
                        source: LastHopRouterSource::DestinationUnreachable { kind: *kind },
                    })
                }
                EchoReply {
                    different_from: _,
                    sent_ttl,
                } => {
                    // TODO handle different from somehow ?
                    result
                        .follow_ups
                        .push(TraceResponsive {
                            targets: response.intended_targets.clone(),
                            sent_ttl: *sent_ttl,
                        })
                }
                Other { description } => result.weird_behaviours.push(OtherWeird {
                    description: description.to_string(),
                }),
                NoResponse => unresponsive_addrs.extend(&response.intended_targets),
                TimeExceeded { from: _, sent_ttl } => {
                    result.weird_behaviours.push(TtlExceeded {
                        sent_ttl: *sent_ttl,
                    })
                }
            }
        }
        let nothing_else_recorded =
            result.follow_ups.is_empty() && result.last_hop_routers.is_empty();
        if !unresponsive_addrs.is_empty() && nothing_else_recorded {
            result
                .follow_ups
                .push(TraceUnresponsive {
                    candidates: unresponsive_addrs,
                })
        }
        result
    }
}
