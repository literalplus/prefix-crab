use anyhow::*;
use chrono::{NaiveDateTime, Utc};
use diesel::dsl::*;
use diesel::prelude::*;
use diesel::upsert::excluded;
use diesel::PgConnection;
use log::info;
use log::warn;


use crate::models::analysis;
use crate::models::analysis::FollowUp;
use crate::models::analysis::LastHopRouterData;

use super::interpret::model::CanFollowUp;
use super::interpret::model::EchoSplitResult;
use super::interpret::model::FollowUpTraceRequest;
use super::interpret::model::LastHopRouter;
use super::interpret::model::LastHopRouterSource::{self, *};
use super::{context::ProbeContext, interpret::model::EchoResult};

use crate::models::analysis::SplitAnalysisDetails;
use crate::models::analysis::SplitData;
use crate::models::analysis::Stage;
use crate::models::tree::PrefixTree;
use crate::schema::split_analysis::dsl as analysis_dsl;
use crate::schema::split_analysis_split::dsl as split_dsl;
use queue_models::echo_response::DestUnreachKind::*;

pub fn create_analysis(
    conn: &mut PgConnection,
    node: &PrefixTree,
    split_prefix_len: u8,
) -> Result<()> {
    insert_into(analysis_dsl::split_analysis)
        .values((
            analysis_dsl::tree_id.eq(&node.id),
            analysis_dsl::split_prefix_len.eq(split_prefix_len as i16),
        ))
        .execute(conn)?;
    Ok(())
}

pub fn update_analysis_with_echo(
    conn: &mut PgConnection,
    interpretation: EchoResult,
    context: &mut ProbeContext,
) -> Result<()> {
    let log_id = context.log_id();
    let details = match &mut context.analyses.active {
        None => {
            warn!(
                "Tried to update with analysis {:?} but none was active.",
                interpretation
            );
            return Ok(());
        }
        Some(active) => active,
    };
    for model in &interpretation.splits {
        let work_split = &mut details[&model.net_index];
        update_split_data(model, &mut work_split.data);
    }
    let parent_update = determine_parent_update(details, log_id);
    conn.transaction(|conn| {
        diesel::update(&details.analysis)
            .set(parent_update)
            .execute(conn)?;
        diesel::insert_into(split_dsl::split_analysis_split)
            .values(details.borrow_splits())
            .on_conflict((split_dsl::analysis_id, split_dsl::net_index))
            .do_update()
            .set(split_dsl::data.eq(excluded(split_dsl::data)))
            .execute(conn)?;
        Ok(())
    })
    .context("while saving changes")?;
    Ok(())
}

fn determine_parent_update(
    details: &mut SplitAnalysisDetails,
    log_id: String,
) -> AnalysisStageUpdateDueToEcho {
    let mut parent = details.analysis;
    if details.needs_follow_up() {
        parent.stage = Stage::PendingTrace;
        info!("Follow-up traces necessary for {}", log_id);
        AnalysisStageUpdateDueToEcho {
            stage: Stage::PendingTrace,
            completed_at: None,
        }
    } else {
        parent.stage = Stage::Completed;
        info!("Data collection is complete for {}", log_id);
        AnalysisStageUpdateDueToEcho {
            stage: Stage::Completed,
            completed_at: Some(Utc::now().naive_utc()),
        }
    }
}

#[derive(AsChangeset)]
#[diesel(table_name = crate::schema::split_analysis)]
struct AnalysisStageUpdateDueToEcho {
    stage: Stage,
    completed_at: Option<NaiveDateTime>,
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
