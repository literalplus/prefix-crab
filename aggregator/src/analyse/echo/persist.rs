use anyhow::{Context as AnyhowContext, *};

use diesel::prelude::*;
use diesel::PgConnection;
use ipnet::Ipv6Net;
use itertools::Itertools;
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

use crate::persist::dsl::CidrMethods;
use crate::persist::DieselErrorFixCause;

use crate::prefix_tree::context::ContextOps;
use crate::schedule::FollowUpId;
use crate::schema::measurement_tree::dsl::measurement_tree;
use crate::schema::measurement_tree::target_net;

impl UpdateAnalysis for EchoResult {
    fn update_analysis(mut self, conn: &mut PgConnection, context: &mut Context) -> Result<()> {
        //              ^   Need move receiver due to `drain()` called in `save()`

        let log_id = context.log_id();

        let update = self.determine_parent_update(&mut context.analysis, log_id);
        let forest = self.drain_to_measurement_forest()?;
        Self::save(conn, &context.analysis, update, forest)
    }
}

impl EchoResult {
    fn determine_parent_update(
        &self,
        parent: &mut SplitAnalysis,
        log_id: String,
    ) -> RegisterFollowUpChangeset {
        if self.needs_follow_up() {
            let id = FollowUpId::new();
            parent.pending_follow_up = Some(id.to_string());
            info!("Follow-up traces necessary for {}", log_id);
            RegisterFollowUpChangeset {
                pending_follow_up: Some(id.to_string()),
            }
        } else {
            info!(
                "Data collection is complete for {}, not follow-up necessary.",
                log_id
            );
            // Removal of an existing follow-up should be done by the code handling follow-up responses
            RegisterFollowUpChangeset {
                pending_follow_up: None,
            }
        }
    }

    fn save(
        conn: &mut PgConnection,
        analysis: &SplitAnalysis,
        update: RegisterFollowUpChangeset,
        forest: MeasurementForest,
    ) -> Result<()> {
        conn.transaction(|conn| {
            let relevant_measurements = load_relevant_measurements(conn, analysis, &forest)?;
            save_merging_into_existing(conn, relevant_measurements, forest)?;

            if update.has_changes() {
                diesel::update(analysis).set(update).execute(conn)?;
            }
            
            Ok(())
        })
        .context("while saving changes")
    }

    fn drain_to_measurement_forest(&mut self) -> Result<MeasurementForest> {
        let mut forest = MeasurementForest::default();
        for (net, entry) in self.drain() {
            forest.insert(make_tree(net, entry))?;
        }
        Ok(forest)
    }
}

fn load_relevant_measurements(
    conn: &mut PgConnection,
    analysis: &SplitAnalysis,
    forest: &MeasurementForest,
) -> Result<Vec<MeasurementTree>> {
    let mut query = measurement_tree.into_boxed();
    for net in forest.to_iter_all_nets() {
        query = query.or_filter(target_net.supernet_or_eq6(&net));
    }
    query.load(conn).fix_cause().with_context(|| {
        format!(
            "while loading existing trees for amendment related to PrefixTree[{}], \n\
            with potential MeasurementTree prefixes: {:?}.",
            analysis.tree_net,
            forest.to_iter_all_nets().collect_vec(),
        )
    })
}

fn save_merging_into_existing(
    conn: &mut PgConnection,
    relevant_measurements: Vec<MeasurementTree>,
    local_forest: MeasurementForest,
) -> Result<()> {
    let num_trees = relevant_measurements.len();
    let mut remote_forest = MeasurementForest::with_untouched(relevant_measurements)?;
    trace!("Remote forest has {} trees: {}", num_trees, remote_forest);
    for tree_from_result in local_forest.into_iter_touched() {
        remote_forest.insert(tree_from_result.tree)?
    }
    let obsolete_nets: Vec<&Ipv6Net> = remote_forest.obsolete_nets.iter().collect();
    if !obsolete_nets.is_empty() {
        warn!(
            "Encountered obsolete measurement nodes: {:?}",
            obsolete_nets
        );
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
                    .execute(conn)
                    .fix_cause()?;
            }
        }
    }
    if !inserts.is_empty() {
        trace!(
            "Inserting {} FRESH trees for the CO2 credits",
            inserts.len()
        );
        diesel::insert_into(measurement_tree)
            .values(inserts)
            .on_conflict_do_nothing()
            .execute(conn)
            .fix_cause()?;
    }
    Ok(())
}

fn make_tree(net: Ipv6Net, entry: PrefixEntry) -> MeasurementTree {
    let mut tree = MeasurementTree::empty(net);
    for (addr, LastHopRouter { sources, hit_count }) in entry.last_hop_routers.into_iter() {
        tree.add_lhr_no_sum(addr, sources, hit_count);
    }
    for (description, node) in entry.weird.into_iter() {
        tree.add_weird_no_sum(description, node.hit_count);
    }
    tree.responsive_count = entry.responsive_count;
    tree.unresponsive_count = entry.unresponsive_count;
    tree
}

#[derive(AsChangeset)]
#[diesel(table_name = crate::schema::split_analysis)]
struct RegisterFollowUpChangeset {
    pending_follow_up: Option<String>,
}

impl RegisterFollowUpChangeset {
    fn has_changes(&self) -> bool {
        self.pending_follow_up.is_some()
    }
}
