use anyhow::{Context as AnyhowContext, *};
use diesel::insert_into;
use diesel::prelude::*;
use ipnet::Ipv6Net;

use crate::persist::dsl::CidrMethods;
use crate::persist::DieselErrorFixCause;
use crate::prefix_tree::PrefixTree;
use crate::schema::prefix_tree::dsl::*;

#[derive(Debug)]
pub struct Context {
    pub node: PrefixTree,
}

impl From<PrefixTree> for Context {
    fn from(node: PrefixTree) -> Self {
        Context{ node }
    }
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

    let node = select_self(conn, target_net)?;
    Result::Ok(Context { node })
}

fn select_self(conn: &mut PgConnection, target_net: &Ipv6Net) -> Result<PrefixTree> {
    let tree = prefix_tree
        .filter(net.eq6(target_net))
        .order_by(net)
        .get_result(conn)
        .fix_cause()
        .with_context(|| "while selecting prefix tree")?;
    Ok(tree)
}

fn insert_if_new(conn: &mut PgConnection, target_net: &Ipv6Net) -> Result<()> {
    insert_into(prefix_tree)
        .values((net.eq6(target_net), is_routed.eq(true)))
        .on_conflict_do_nothing()
        .execute(conn)
        .with_context(|| "while trying to insert into prefix_tree")?;
    Ok(())
}
