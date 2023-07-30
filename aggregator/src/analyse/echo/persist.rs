use anyhow::{Context as AnyhowContext, *};
use chrono::{NaiveDateTime, Utc};

use diesel::prelude::*;
use diesel::upsert::excluded;
use diesel::PgConnection;
use log::info;

use crate::analyse::MeasurementTree;
use crate::analyse::context::Context;
use crate::analyse::persist::UpdateAnalysis;
use crate::analyse::CanFollowUp;
use crate::analyse::EchoResult;
use crate::analyse::SplitAnalysis;

use crate::analyse::Stage;
use crate::prefix_tree::context::ContextOps;

impl UpdateAnalysis for EchoResult {
    fn update_analysis(&self, conn: &mut PgConnection, context: &mut Context) -> Result<()> {
        let log_id = context.log_id();
        let active = self.extract_active_or_fail(context)?;

        // TODO need to update LHRs separately; it doesn't provide an additional benefit to store them
        // in the split analysis as well (other than maybe ergonomics), and we can immediately commit
        // them to the prefix_lhr table.

        let update = self.determine_parent_update(active, log_id);
        Self::save(conn, active, update)
    }
}

impl EchoResult {
    fn extract_active_or_fail<'a>(
        &self,
        context: &'a mut Context,
    ) -> Result<&'a mut SplitAnalysis> {
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

    fn determine_parent_update(&self, parent: &mut SplitAnalysis, log_id: String) -> ParentUpdate {
        if self.needs_follow_up() {
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

    fn save(conn: &mut PgConnection, analysis: &SplitAnalysis, update: ParentUpdate) -> Result<()> {
        conn.transaction(|conn| {
            diesel::update(analysis).set(update).execute(conn)?;
            // TODO update measurement tree
            Ok(())
        })
        .context("while saving changes")
    }

    fn to_measurement_trees(&self) -> Vec<MeasurementTree> {
        todo!()
    }
}

#[derive(AsChangeset)]
#[diesel(table_name = crate::schema::split_analysis)]
struct ParentUpdate {
    stage: Stage,
    completed_at: Option<NaiveDateTime>,
}

// TODO
// fn update_lhr(model: &LastHopRouter, data_routers: &mut Vec<LastHopRouterData>) {
//     let existing = data_routers
//         .iter_mut()
//         .find(|it| it.address == model.address);
//     match existing {
//         Some(it) => {
//             it.hits += model.hit_count as i32;
//             if model.source != it.source {
//                 info!("LHR encountered via different source {:?}", model.source);
//             }
//         }
//         None => {
//             data_routers.push(LastHopRouterData {
//                 address: model.address,
//                 source: model.source,
//                 hits: model.hit_count as i32,
//             });
//         }
//     }
// }
