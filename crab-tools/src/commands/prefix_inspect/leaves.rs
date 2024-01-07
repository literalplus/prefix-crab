pub use component::Leaves;
use db_model::{
    persist::DieselErrorFixCause,
    persist::{self, dsl::CidrMethods},
    prefix_tree::{MergeStatus, PrefixTree},
};
use diesel::{prelude::*, PgConnection};
use ipnet::Ipv6Net;
use itertools::Itertools;
use model::*;

macro_rules! deterr {
    ($which:ident) => {
        |source| Error::$which {
            desc: format!("{:?}", source),
        }
    };
}

mod component;
mod model;

pub fn find_leaves(net: &Ipv6Net) -> Result {
    let mut conn = persist::connect().map_err(deterr!(DbConnect))?;

    let mut leaves = load_leaves(&mut conn, net)?
        .into_iter()
        .map_into()
        .collect_vec();

    leaves.sort_by_key(|pfx: &LeafNet| pfx.net);
    mark_redundant_neighbors(&mut leaves);

    Ok(leaves)
}

fn load_leaves(conn: &mut PgConnection, supernet: &Ipv6Net) -> StdResult<Vec<PrefixTree>, Error> {
    use db_model::schema::prefix_tree::dsl::*;

    prefix_tree
        .filter(
            net.subnet_or_eq6(supernet)
                .and(merge_status.eq_any(&[MergeStatus::Leaf, MergeStatus::UnsplitRoot])),
        )
        .select(PrefixTree::as_select())
        .load(conn)
        .fix_cause()
        .map_err(deterr!(LoadTree))
}

fn mark_redundant_neighbors(leaves: &mut Vec<LeafNet>) {
    // Check for all possible adjacent pairs if they are redundant neighbours
    for left_idx in 0..(leaves.len() - 1) {
        let right_idx = left_idx + 1;
        let (left, right) = (&leaves[left_idx], &leaves[right_idx]);
        if !are_neighbors(&left.net, &right.net) {
            continue;
        }
        if left.priority_class == right.priority_class {
            // Note that this isn't 100% accurate as we would need to compare the LHRs & ratios,
            // but it can at least provide an indication where to look for flase-positive merges
            leaves[left_idx].redundant = true;
            leaves[right_idx].redundant = true;
        }
    }
}

fn are_neighbors(left: &Ipv6Net, right: &Ipv6Net) -> bool {
    left.prefix_len() == right.prefix_len() && left.supernet() == right.supernet()
}
