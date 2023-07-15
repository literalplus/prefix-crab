use anyhow::*;
use diesel::dsl::*;
use diesel::prelude::*;
use diesel::PgConnection;
use log::info;
use log::warn;

use crate::models::analysis;
use crate::models::analysis::FollowUp;
use crate::models::analysis::LastHopRouterData;

use crate::models::analysis::SplitAnalysis;
use crate::models::analysis::SplitData;
use crate::models::tree::PrefixTree;
use crate::schema::split_analysis::dsl::*;

use super::interpret::model::EchoSplitResult;
use super::interpret::model::FollowUpTraceRequest;
use super::interpret::model::LastHopRouter;
use super::interpret::model::LastHopRouterSource::{self, *};
use super::{context::ProbeContext, interpret::model::EchoResult};
use queue_models::echo_response::DestUnreachKind::*;

pub fn create_analysis(
    conn: &mut PgConnection,
    node: &PrefixTree,
    prefix_len: i8,
) -> Result<SplitAnalysis> {
    let inserted = insert_into(split_analysis)
        .values((tree_id.eq(&node.id), split_prefix_len.eq(prefix_len as i16)))
        .get_result(conn)?;
    Ok(inserted)
}

pub fn update_analysis_with_echo(
    _conn: &mut PgConnection,
    interpretation: EchoResult,
    context: ProbeContext,
) {
    let details = match context.analyses.active {
        None => {
            warn!(
                "Tried to update with analysis {:?} but none was active.",
                interpretation
            );
            return;
        }
        Some(active) => active,
    };
    for split in &interpretation.splits {
        // TODO correlate split-ID with prefix -> extract split logic to common
    }
    // for each split:
    //  - find its corresponding entitiy
    //  - if missing, mark it to be inserted somehow
    //  - if existing, update it later
    // if no follow ups, mark completed
}

fn update_split_data(model: &EchoSplitResult, data: &mut SplitData) {
    for lhr in &model.last_hop_routers {
        update_lhr(lhr, &mut data.last_hop_routers);
    }
    for weird in &model.weird_behaviours {
        data.weird_behaviours.insert(weird.get_id());
    }
    for follow_up in &model.follow_ups {
        data.pending_follow_ups.push(follow_up.into())
    }
}

fn update_lhr(model: &LastHopRouter, data_routers: &mut Vec<LastHopRouterData>) {
    let existing = data_routers
        .iter_mut()
        .find(|it| it.address == model.router);
    let new_source = (&model.source).into();
    match existing {
        Some(it) => {
            it.hits += model.get_hit_count() as i32;
            if new_source != it.source {
                info!("LHR encountered via different source {:?}", model.source);
            }
        }
        None => {
            data_routers.push(LastHopRouterData {
                address: model.router.clone(),
                source: new_source,
                hits: model.get_hit_count() as i32,
            });
        }
    }
}

impl From<&LastHopRouterSource> for analysis::LastHopRouterSource {
    fn from(value: &LastHopRouterSource) -> Self {
        match value {
            TraceUnresponsive => analysis::LastHopRouterSource::TraceUnresponsive,
            TraceResponsive => analysis::LastHopRouterSource::TraceResponsive,
            DestinationUnreachable { kind } => match kind {
                NoRoute => analysis::LastHopRouterSource::DestUnreachReject,
                AdminProhibited => analysis::LastHopRouterSource::DestUnreachProhibit,
                Other(_) => todo!(),
                AddressUnreachable => analysis::LastHopRouterSource::DestUnreachAddrPort,
                PortUnreachable => analysis::LastHopRouterSource::DestUnreachAddrPort,
            },
        }
    }
}

impl From<&FollowUpTraceRequest> for FollowUp {
    fn from(value: &FollowUpTraceRequest) -> Self {
        match value {
            FollowUpTraceRequest::TraceResponsive { targets, sent_ttl } => {
                FollowUp::TraceResponsive {
                    targets: targets.clone(),
                    sent_ttl: *sent_ttl,
                }
            }
            FollowUpTraceRequest::TraceUnresponsive { candidates } => FollowUp::TraceUnresponsive {
                candidates: candidates.clone(),
            },
        }
    }
}
