use anyhow::{*, Context as AnyhowContext};
use diesel::dsl::not;
use diesel::insert_into;
use diesel::prelude::*;
use ipnet::Ipv6Net;

use crate::persist::dsl::CidrMethods;
use crate::prefix_tree::PrefixTree;
use crate::schema::prefix_tree::dsl::*;
use crate::persist::DieselErrorFixCause;

#[derive(Debug)]
pub struct Context {
    pub node: PrefixTree,
    pub ancestors: Vec<PrefixTree>,
    pub unmerged_children: Vec<PrefixTree>,
}

pub trait ContextOps {
    fn log_id(&self) -> String;
    fn node(&self) -> &PrefixTree;
}

impl ContextOps for Context {
    fn log_id(&self) -> String {
        self.node.net.to_string()
    }

    fn node(&self) -> &PrefixTree {
        &self.node
    }
}

pub fn fetch(conn: &mut PgConnection, target_net: &Ipv6Net) -> Result<Context> {
    insert_if_new(conn, target_net)?;

    let ancestors_and_self = select_ancestors_and_self(conn, target_net)
        .with_context(|| "while finding ancestors and self")?;
    let (ancestors, node) = match &ancestors_and_self[..] {
        [parents @ .., node] => (parents.to_vec(), *node),
        [] => bail!("Didn't find the prefix_tree node we just inserted :("),
    };
    Result::Ok(Context {
        node,
        ancestors,
        unmerged_children: select_unmerged_children(conn, &node)?,
    })
}

fn select_ancestors_and_self(
    connection: &mut PgConnection,
    target_net: &Ipv6Net,
) -> Result<Vec<PrefixTree>> {
    let parents = prefix_tree
        .filter(net.supernet_or_eq6(target_net))
        //.select(PrefixTree::as_select())
        .order_by(net)
        .load(connection)
        .fix_cause()
        .with_context(|| "while selecting parents")?;
    Ok(parents)
}

fn select_unmerged_children(
    connection: &mut PgConnection,
    node: &PrefixTree,
) -> Result<Vec<PrefixTree>> {
    let parents = prefix_tree
        .filter(net.subnet_or_eq6(&node.net))
        .filter(not(net.eq6(&node.net)))
        //.select(PrefixTree::as_select())
        .order_by(net)
        .load(connection)
        .fix_cause()
        .with_context(|| "while selecting unmerged children")?;
    Ok(parents)
}

fn insert_if_new(connection: &mut PgConnection, target_net: &Ipv6Net) -> Result<()> {
    insert_into(prefix_tree)
        .values((
            net.eq6(target_net),
            is_routed.eq(true),
        ))
        .on_conflict_do_nothing()
        .execute(connection)
        .with_context(|| "while trying to insert into prefix_tree")?;
    Ok(())
}
