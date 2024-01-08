use anyhow::Result;
use db_model::{
    analyse::{forest::MeasurementForest, MeasurementTree},
    persist::{dsl::CidrMethods, DieselErrorFixCause},
    prefix_tree::ContextOps,
};
use diesel::dsl::*;
use diesel::{query_dsl::methods::FilterDsl, Connection, PgConnection, RunQueryDsl};
use ipnet::Ipv6Net;
use itertools::Itertools;

use crate::analyse::context::Context;

pub fn process(
    conn: &mut PgConnection,
    request: &Context,
    relevant_measurements: Vec<MeasurementTree>,
) -> Result<()> {
    let base_net = request.node().net;
    let new_prefix_len = (base_net.prefix_len() + 4).clamp(0, 64);
    let mut forest = MeasurementForest::default();
    for subnet in base_net.subnets(new_prefix_len)? {
        // Pre-fill larger merged nets s.t. no /64s are inserted below
        forest.insert(MeasurementTree::empty(subnet))?;
    }
    for tree in relevant_measurements {
        forest.insert(tree)?;
    }

    delete_existing_save_merged(conn, base_net, forest)
}

fn delete_existing_save_merged(
    conn: &mut PgConnection,
    base_net: Ipv6Net,
    forest: MeasurementForest,
) -> Result<()> {
    use db_model::schema::measurement_tree::dsl::*;

    conn.transaction(|conn| {
        delete(measurement_tree.filter(target_net.subnet_or_eq6(&base_net)))
            .execute(conn)
            .fix_cause()?;

        // Batching would be ideal, but Diesel doesn't seem to directly support that
        // ref: https://github.com/diesel-rs/diesel/issues/1517
        let inserts = forest
            .into_iter()
            .filter(|it| !it.tree.is_empty())
            .map(|it| it.tree)
            .collect_vec();
        diesel::insert_into(measurement_tree)
            .values(inserts)
            .on_conflict_do_nothing()
            .execute(conn)
            .fix_cause()?;

        Ok(())
    })
}
