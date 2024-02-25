use anyhow::{Context as AnyhowContext, *};

use db_model::analyse::MeasurementTree;
use diesel::prelude::*;
use diesel::sql_types::Array;
use diesel::sql_types::Cidr;
use diesel::sql_types::Integer;
use diesel::sql_types::Jsonb;
use diesel::PgConnection;
use ipnet::Ipv6Net;
use itertools::Itertools;
use log::trace;
use log::warn;
use tracing::instrument;
use tracing::Span;

use crate::analyse::context::Context;
use crate::analyse::Interpretation;
use crate::analyse::LastHopRouter;
use crate::analyse::MeasurementForest;
use crate::analyse::ModifiableTree;
use crate::analyse::ModificationType;
use crate::analyse::Prefix;
use crate::analyse::SplitAnalysis;

use crate::persist::dsl::CidrMethods;
use crate::persist::DieselErrorFixCause;

use crate::schema::measurement_tree::dsl::measurement_tree;
use crate::schema::measurement_tree::target_net;

use diesel::dsl::*;

pub fn begin_bulk(conn: &mut PgConnection, nets: &[Ipv6Net]) -> Result<()> {
    use crate::schema::split_analysis::dsl::*;

    let tuples = nets.iter().map(|net| tree_net.eq6(net)).collect_vec();
    insert_into(split_analysis)
        .values(tuples)
        .execute(conn)
        .fix_cause()?;
    Ok(())
}

pub trait UpdateAnalysis {
    fn update_analysis(&mut self, conn: &mut PgConnection, context: &mut Context) -> Result<()>;
}

impl UpdateAnalysis for Interpretation {
    #[instrument(skip_all)]
    fn update_analysis(&mut self, conn: &mut PgConnection, context: &mut Context) -> Result<()> {
        save(conn, &context.analysis, self.drain_to_measurement_forest()?)
    }
}

impl Interpretation {
    fn drain_to_measurement_forest(&mut self) -> Result<MeasurementForest> {
        let mut forest = MeasurementForest::default();
        for (net, entry) in self.drain() {
            forest.insert(make_tree(net, entry))?;
        }
        Ok(forest)
    }
}

fn save(
    conn: &mut PgConnection,
    analysis: &SplitAnalysis,
    forest: MeasurementForest,
) -> Result<()> {
    conn.transaction(|conn| {
        let relevant_measurements = load_relevant_measurements(conn, analysis, &forest)?;
        save_merging_into_existing(conn, relevant_measurements, forest)?;
        Ok(())
    })
    .context("while saving changes")
}

#[instrument(skip_all, fields(forest = %forest, found_trees, filter_trees64))]
fn load_relevant_measurements(
    conn: &mut PgConnection,
    analysis: &SplitAnalysis,
    forest: &MeasurementForest,
) -> Result<Vec<MeasurementTree>> {
    if forest.is_empty() {
        return Ok(vec![]);
    }

    // TODO: This is a bit of a bottleneck; Instead we could save everything as /64 and rely on the merge logic with
    // high confidence to normalise cases where a supernet exists and the /64 as well
    let mut query = measurement_tree.into_boxed();

    Span::current().record("filter_trees64", forest.get_trees64_count());
    if forest.get_trees64_count() > 10 {
        // For large numbers of /64 to look up, supernet queries take really long to execute.
        // In these cases, load all trees of the target net instead, which is  in experience much faster to evaluate,
        // even though it might yield many more results.
        //
        // An alternative could also be to just always safe /64s and only merge them while actually merging.
        query = query.filter(target_net.subnet_or_eq6(&analysis.tree_net));
    } else {
        for net in forest.to_iter_all_nets() {
            // NOTE: This is for the _super_net, i.e. we want to find the /64s that we hit AND any potentially-merged
            // supernets that we'd need to update instead.
            query = query.or_filter(target_net.supernet_or_eq6(&net));
        }
    }
    let res = query.load(conn).fix_cause().with_context(|| {
        format!(
            "while loading existing trees for amendment related to PrefixTree[{}], \n\
            with potential MeasurementTree prefixes: {:?}.",
            analysis.tree_net,
            forest.to_iter_all_nets().collect_vec(),
        )
    })?;
    Span::current().record("found_trees", res.len());
    Ok(res)
}

#[instrument(skip_all)]
fn save_merging_into_existing(
    conn: &mut PgConnection,
    relevant_measurements: Vec<MeasurementTree>,
    local_forest: MeasurementForest,
) -> Result<()> {
    let num_trees = relevant_measurements.len();
    let mut remote_forest = MeasurementForest::with_untouched(relevant_measurements)?;
    trace!("Remote forest has {} trees: {}", num_trees, remote_forest);
    for tree_from_result in local_forest.into_iter() {
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
    let mut updates = vec![];
    for updated_tree in remote_forest.into_iter() {
        let ModifiableTree { tree, touched } = updated_tree;
        match touched {
            ModificationType::Untouched => {}
            ModificationType::Inserted => inserts.push(tree),
            ModificationType::Updated => {
                updates.push(tree);
                // TODO: Since this is a major % in the trace, maybe parallelise them if we can't batch
                // for that we need a connection pool!
                // diesel::update(measurement_tree)
                //     .filter(target_net.eq(tree.target_net))
                //     .set(tree)
                //     .execute(conn)
                //     .fix_cause()?;
                // update_count += 1;
            }
        }
    }

    if !inserts.is_empty() {
        do_inserts(conn, inserts)?;
    }

    if !updates.is_empty() {
        do_updates(conn, updates)?;
    }

    Ok(())
}

#[instrument(skip_all, fields(inserts = inserts.len()))]
fn do_inserts(conn: &mut PgConnection, inserts: Vec<MeasurementTree>) -> Result<usize> {
    trace!(
        "Inserting {} FRESH trees for the CO2 credits",
        inserts.len()
    );
    diesel::insert_into(measurement_tree)
        .values(inserts)
        .on_conflict_do_nothing()
        .execute(conn)
        .fix_cause()
}

#[instrument(skip_all, fields(updates = updates.len()))]
fn do_updates(conn: &mut PgConnection, updates: Vec<MeasurementTree>) -> Result<usize> {
    let query = diesel::sql_query(
        "
        UPDATE measurement_tree mt
        SET updated_at = NOW(),
            responsive_count = dat.responsive_count,
            unresponsive_count = dat.unresponsive_count,
            last_hop_routers = dat.last_hop_routers,
            weirdness = dat.weirdness
        FROM (
            SELECT
                UNNEST($1) as target_net,
                UNNEST($2) as responsive_count, UNNEST($3) as unresponsive_count,
                UNNEST($4) as last_hop_routers, UNNEST($5) as weirdness
        ) dat
        WHERE mt.target_net = dat.target_net
    ",
    );

    let query = query
        .bind::<Array<Cidr>, _>(updates.iter().map(|it| it.target_net).collect_vec())
        .bind::<Array<Integer>, _>(updates.iter().map(|it| it.responsive_count).collect_vec())
        .bind::<Array<Integer>, _>(updates.iter().map(|it| it.unresponsive_count).collect_vec())
        .bind::<Array<Jsonb>, _>(updates.iter().map(|it| it.last_hop_routers.clone()).collect_vec())
        .bind::<Array<Jsonb>, _>(updates.into_iter().map(|it| it.weirdness).collect_vec())
        ;

    query.execute(conn).fix_cause()
}

fn make_tree(net: Ipv6Net, entry: Prefix) -> MeasurementTree {
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
