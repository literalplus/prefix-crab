use anyhow::*;
use diesel::dsl::*;
use diesel::prelude::*;
use diesel::PgConnection;
use log::info;
use log::warn;

use crate::models::analysis;
use crate::models::analysis::LastHopRouterData;
use crate::models::analysis::Split;
use crate::models::analysis::SplitAnalysis;
use crate::models::analysis::SplitData;
use crate::models::tree::PrefixTree;
use crate::schema::split_analysis::dsl::*;

use super::interpret::model::EchoSplitResult;
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
    conn: &mut PgConnection,
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
    // for each split:
    //  - find its corresponding entitiy
    //  - if missing, mark it to be inserted somehow
    //  - if existing, update it later
    // if no follow ups, mark completed
}

fn update_split_data(
    conn: &mut PgConnection,
    model: &EchoSplitResult,
    data: &mut SplitData,
) {
    for lhr in &model.last_hop_routers {
        let existing = data
            .last_hop_routers
            .iter_mut()
            .find(|it| it.address == lhr.router);
        let new_source = (&lhr.source).into();
        match existing {
            Some(it) => {
                it.hits += lhr.get_hit_count() as i32;
                if new_source != it.source {
                    info!("LHR encountered via different source {:?}", lhr.source);
                }
            }
            None => {
                data.last_hop_routers.push(LastHopRouterData {
                    address: lhr.router.clone(),
                    source: new_source,
                    hits: lhr.get_hit_count() as i32,
                });
            }
        }
    }
    for weird in &model.weird_behaviours {
        data.weird_behaviours.insert(weird.get_id());
    }
    // TODO store follow ups
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
