use diesel::prelude::*;
use ipnet::Ipv6Net;
use prefix_crab::error::IsPermanent;
use thiserror::Error;
use tracing::{instrument, Span};

use crate::persist::dsl::CidrMethods;
use crate::persist::DieselErrorFixCause;
use crate::prefix_tree::PrefixTree;

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

#[derive(Error, Debug)]
pub enum ContextFetchError {
    #[error("this net is not in the prefix tree: {net:?}")]
    NotInPrefixTree { net: Ipv6Net },
    
    #[error("problem talking to the database")]
    DbError(#[from] anyhow::Error),
}

impl IsPermanent for ContextFetchError {
    fn is_permanent(&self) -> bool {
        match self {
            ContextFetchError::NotInPrefixTree { net: _ } => true,
            ContextFetchError::DbError(_) => false,
        }
    }
}

pub type ContextFetchResult = Result<Context, ContextFetchError>;

#[instrument(name = "fetch tree ctx", skip(conn), fields(asn))]
pub fn fetch(conn: &mut PgConnection, net: &Ipv6Net) -> ContextFetchResult {
    let node = select_self(conn, net)?;
    Span::current().record("asn", format!("{}", node.asn));
    Result::Ok(Context { node })
}

fn select_self(conn: &mut PgConnection, target_net: &Ipv6Net) -> Result<PrefixTree, ContextFetchError> {
    use crate::schema::prefix_tree::dsl::*;

    let tree = prefix_tree
        .filter(net.eq6(target_net))
        .order_by(net)
        .get_results(conn)
        .fix_cause()
        .map_err(ContextFetchError::from)?;

    if tree.len() != 1 {
        Err(ContextFetchError::NotInPrefixTree { net: *target_net })
    } else {
        Ok(tree.into_iter().next().expect("vec with one entry to yield that entry"))
    }
}
