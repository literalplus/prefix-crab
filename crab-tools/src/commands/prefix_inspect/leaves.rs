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
