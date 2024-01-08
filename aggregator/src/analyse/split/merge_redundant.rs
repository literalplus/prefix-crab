use anyhow::{anyhow, bail, Result};
use db_model::{
    persist::dsl::CidrMethods,
    persist::DieselErrorFixCause,
    prefix_tree::{ContextOps, MergeStatus, PrefixTree, PriorityClass},
};
use diesel::{dsl::sql, prelude::*, BoolExpressionMethods, ExpressionMethods, PgConnection};
use ipnet::Ipv6Net;
use itertools::Itertools;
use log::info;

use crate::analyse::context::Context;

pub fn process(conn: &mut PgConnection, request: &Context) -> Result<()> {
    if is_redundant(conn, request)? {
        info!(
            "Prefix {} is redundant, merging with sibling",
            request.node().net
        );
        merge(conn, request)?;
    }
    Ok(())
}

pub fn is_redundant(conn: &mut PgConnection, request: &Context) -> Result<bool> {
    use db_model::schema::prefix_tree::dsl::*;

    let nets = AdjacentNets::try_from(request.node().net)?;

    let found_adjacents: Vec<PrefixTree> = prefix_tree
        .or_filter(
            net.eq6(&nets.sibling)
                .and(priority_class.eq(request.node().priority_class))
                .and(confidence.ge(100))
                .and(merge_status.eq(MergeStatus::Leaf)),
        )
        .or_filter(
            net.eq6(&nets.parent)
                .and(merge_status.ne(MergeStatus::Blocked)),
        )
        .load(conn)
        .fix_cause()?;

    Ok(found_adjacents.len() == 2) // only OK if sibling & parent exist and are valid
}

struct AdjacentNets {
    own: Ipv6Net,
    parent: Ipv6Net,
    sibling: Ipv6Net,
}

impl TryFrom<Ipv6Net> for AdjacentNets {
    type Error = anyhow::Error;

    fn try_from(value: Ipv6Net) -> Result<Self> {
        let parent = value.supernet().ok_or_else(|| anyhow!("must not be /1"))?;
        let sibling = parent
            .subnets(value.prefix_len())?
            .filter(move |candidate| candidate != &value)
            .at_most_one()?
            .ok_or_else(|| anyhow!("not all subnets should be the same"))?;
        Ok(Self {
            own: value,
            parent,
            sibling,
        })
    }
}

pub fn merge(conn: &mut PgConnection, request: &Context) -> Result<()> {
    use db_model::schema::prefix_tree::dsl::*;

    let nets = AdjacentNets::try_from(request.node().net)?;

    conn.transaction(|conn| {
        let n_updated_children = diesel::update(prefix_tree)
            .filter(
                merge_status
                    .eq(MergeStatus::Leaf)
                    .and(net.eq6(&nets.own).or(net.eq6(&nets.sibling))),
            )
            .set(merge_status.eq(MergeStatus::MergedUp))
            .execute(conn)
            .fix_cause()?;
        if n_updated_children != 2 {
            bail!(
                "One of the children didn't exist or wasn't a leaf any more - {}",
                n_updated_children
            );
        }
        let n_updated_parent = diesel::update(prefix_tree)
            .filter(net.eq6(&nets.parent))
            .set((
                merge_status.eq(sql(
                "CASE WHEN merge_status = 'split_root' THEN 'unsplit_root'::prefix_merge_status ELSE 'leaf' END",
                )), 
                priority_class.eq(PriorityClass::HighFresh), // ensure that the merged net gets analysed soon
                confidence.eq(4), // chosen by fair dice roll, guaranteed to be random
            ))
            .execute(conn)
            .fix_cause()?;
        let _ = MergeStatus::UnsplitRoot.split(); // indicator that this logic is also implemented in the raw SQL above

        if n_updated_parent != 1 {
            bail!("Failed to update parent - {}", n_updated_parent);
        }

        Ok(())
    })
}
