use anyhow::{Context as AnyhowContext, *};
use chrono::{NaiveDateTime, Utc};

use diesel::prelude::*;
use diesel::PgConnection;
use ipnet::Ipv6Net;
use log::info;
use log::trace;
use log::warn;

use crate::analyse::context::Context;
use crate::analyse::persist::UpdateAnalysis;
use crate::analyse::CanFollowUp;
use crate::analyse::EchoResult;
use crate::analyse::LastHopRouter;
use crate::analyse::MeasurementForest;
use crate::analyse::MeasurementTree;
use crate::analyse::ModifiableTree;
use crate::analyse::ModificationType;
use crate::analyse::PrefixEntry;
use crate::analyse::SplitAnalysis;
use crate::analyse::WeirdNode;

use crate::persist::dsl::CidrMethods;

use crate::analyse::Stage;
use crate::prefix_tree::context::ContextOps;
use crate::schema::measurement_tree::dsl::measurement_tree;
use crate::schema::measurement_tree::target_net;

impl UpdateAnalysis for EchoResult {
    fn update_analysis(&self, conn: &mut PgConnection, context: &mut Context) -> Result<()> {
        let log_id = context.log_id();

        let update = self.determine_parent_update(&mut context.analysis, log_id);
        let forest = self.to_measurement_forest()?;
        Self::save(conn, &mut context.analysis, update, forest)
    }
}

impl EchoResult {
    fn determine_parent_update(&self, parent: &mut SplitAnalysis, log_id: String) -> ParentUpdate {
        // TODO also update follow-up id ?
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

    fn save(
        conn: &mut PgConnection,
        analysis: &SplitAnalysis,
        update: ParentUpdate,
        forest: MeasurementForest,
    ) -> Result<()> {
        conn.transaction(|conn| {
            let mut query = measurement_tree.into_boxed();
            for net in forest.to_iter_all_nets() {
                query = query.or_filter(target_net.supernet_or_eq6(&net));
            }
            // trace!("query is {:?}", debug_query(&query));
            let trees = query.load(conn)?;
            let num_trees = trees.len();
            let mut remote_forest = MeasurementForest::with_untouched(trees)?;
            trace!("Remote forest has {} trees: {}", num_trees, remote_forest);
            for tree_from_result in forest.into_iter_touched() {
                remote_forest.insert(tree_from_result.tree)?
            }
            let obsolete_nets: Vec<&Ipv6Net> = remote_forest.obsolete_nets.iter().collect();
            if !obsolete_nets.is_empty() {
                warn!("Encountered obsolete measurement nodes: {:?}", obsolete_nets);
            }
            // Batching would be ideal, but Diesel doesn't seem to directly support that
            // ref: https://github.com/diesel-rs/diesel/issues/1517
            let mut inserts = vec![];
            for updated_tree in remote_forest.into_iter_touched() {
                let ModifiableTree { tree, touched } = updated_tree;
                match touched {
                    ModificationType::Untouched => {}
                    ModificationType::Inserted => inserts.push(tree),
                    ModificationType::Updated => {
                        trace!("Update {}", tree.target_net);
                        diesel::update(measurement_tree)
                            .filter(target_net.eq(tree.target_net))
                            .set(tree)
                            .execute(conn)?;
                    }
                }
            }
            if !inserts.is_empty() {
                trace!("Inserting {} FRESH trees for the CO2 credits", inserts.len());
                diesel::insert_into(measurement_tree)
                    .values(inserts)
                    .on_conflict_do_nothing()
                    .execute(conn)?;
            }

            diesel::update(analysis).set(update).execute(conn)?;
            Ok(())
        })
        .context("while saving changes")
    }

    fn to_measurement_forest(&self) -> Result<MeasurementForest> {
        let mut forest = MeasurementForest::default();
        for (net, entry) in self.iter() {
            forest.insert(make_tree(net, entry))?;
        }
        Ok(forest)
    }
}

fn make_tree(net: Ipv6Net, entry: &PrefixEntry) -> MeasurementTree {
    let mut tree = MeasurementTree::empty(net);
    for (addr, LastHopRouter { sources, hit_count }) in entry.last_hop_routers.iter() {
        tree.add_lhr_no_sum(*addr, sources.clone(), *hit_count);
    }
    for (addr, node) in entry.weird_nodes.iter() {
        let WeirdNode {
            descriptions,
            hit_count,
        } = node;
        tree.add_weird_no_sum(*addr, descriptions.clone(), *hit_count);
    }
    tree.responsive_count = entry.responsive_count;
    tree.unresponsive_count = entry.unresponsive_count;
    tree
}

#[derive(AsChangeset)]
#[diesel(table_name = crate::schema::split_analysis)]
struct ParentUpdate {
    stage: Stage,
    completed_at: Option<NaiveDateTime>,
}
