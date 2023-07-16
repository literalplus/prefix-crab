use anyhow::{Context as AnyhowContext, *};
use chrono::{NaiveDateTime, Utc};

use diesel::prelude::*;
use diesel::upsert::excluded;
use diesel::PgConnection;
use log::info;




use crate::analyse::context::Context;
use crate::analyse::store::UpdateAnalysis;
use crate::analyse::CanFollowUp;
use crate::analyse::EchoResult;
use crate::analyse::EchoSplitResult;
use crate::analyse::LastHopRouter;
use crate::analyse::LastHopRouterData;

use crate::analyse::SplitAnalysisDetails;
use crate::analyse::SplitData;
use crate::analyse::Stage;
use crate::prefix_tree::context::ContextOps;


use crate::schema::split_analysis_split::dsl as split_dsl;

impl UpdateAnalysis for EchoResult {
    fn update_analysis(&self, conn: &mut PgConnection, context: &mut Context) -> Result<()> {
        let log_id = context.log_id();
        let details = self.extract_active_or_fail(context)?;

        for model in &self.splits {
            let work_split = &mut details[&model.net_index];
            update_split_data(model, &mut work_split.data);
        }

        let update = Self::determine_parent_update(details, log_id);
        Self::save(conn, details, update)
    }
}

impl EchoResult {
    fn extract_active_or_fail<'a>(&self, context: &'a mut Context) -> Result<&'a mut SplitAnalysisDetails> {
        match &mut context.active {
            None => {
                bail!(
                    "Tried to update with analysis {:?} but none was active.",
                    self
                );
            }
            Some(active) => Ok(active),
        }
    }

    fn determine_parent_update(details: &mut SplitAnalysisDetails, log_id: String) -> ParentUpdate {
        let mut parent = details.analysis;
        if details.needs_follow_up() {
            parent.stage = Stage::PendingTrace;
            info!("Follow-up traces necessary for {}", log_id);
            ParentUpdate {
                stage: Stage::PendingTrace,
                completed_at: None,
            }
        } else {
            parent.stage = Stage::Completed;
            info!("Data collection is complete for {}", log_id);
            ParentUpdate {
                stage: Stage::Completed,
                completed_at: Some(Utc::now().naive_utc()),
            }
        }
    }

    fn save(
        conn: &mut PgConnection,
        details: &mut SplitAnalysisDetails,
        update: ParentUpdate,
    ) -> Result<()> {
        conn.transaction(|conn| {
            diesel::update(&details.analysis)
                .set(update)
                .execute(conn)?;
            diesel::insert_into(split_dsl::split_analysis_split)
                .values(details.borrow_splits())
                .on_conflict((split_dsl::analysis_id, split_dsl::net_index))
                .do_update()
                .set(split_dsl::data.eq(excluded(split_dsl::data)))
                .execute(conn)?;
            Ok(())
        })
        .context("while saving changes")
    }
}

#[derive(AsChangeset)]
#[diesel(table_name = crate::schema::split_analysis)]
struct ParentUpdate {
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
        data.pending_follow_ups.push(follow_up.clone())
    }
}

fn update_lhr(model: &LastHopRouter, data_routers: &mut Vec<LastHopRouterData>) {
    let existing = data_routers
        .iter_mut()
        .find(|it| it.address == model.address);
    match existing {
        Some(it) => {
            it.hits += model.hit_count as i32;
            if model.source != it.source {
                info!("LHR encountered via different source {:?}", model.source);
            }
        }
        None => {
            data_routers.push(LastHopRouterData {
                address: model.address,
                source: model.source,
                hits: model.hit_count as i32,
            });
        }
    }
}
