use diesel::prelude::*;
use ipnet::{Ipv6Net, IpNet};
use log::warn;
use queue_models::probe_request::TraceRequestId;
use thiserror::Error;

use crate::persist::DieselErrorFixCause;
use crate::persist::dsl::CidrMethods;
use crate::prefix_tree::context::ContextOps;
use crate::prefix_tree::{self, PrefixTree};

use super::SplitAnalysis;

#[derive(Debug)]
pub struct Context {
    pub parent: prefix_tree::Context,
    pub analysis: SplitAnalysis,
}

impl ContextOps for Context {
    fn log_id(&self) -> String {
        self.parent.log_id()
    }

    fn node(&self) -> &PrefixTree {
        &self.parent.node
    }
}

#[derive(Error, Debug)]
pub enum ContextFetchError {
    #[error("no analysis is active for {parent:?}")]
    NoActiveAnalysis { parent: prefix_tree::Context },

    #[error("no analysis is waiting for follow-up trace {id}")]
    NoMatchingAnalysis { id: TraceRequestId },

    #[error("problem talking to the database")]
    DbError(#[from] anyhow::Error),
}

pub type ContextFetchResult = Result<Context, ContextFetchError>;

pub fn fetch(conn: &mut PgConnection, parent: prefix_tree::Context) -> ContextFetchResult {
    let actives = fetch_active(conn, &parent.node)?;
    if actives.is_empty() {
        return Err(ContextFetchError::NoActiveAnalysis { parent });
    } else if actives.len() > 1 {
        warn!("Multiple analyses are active for {}.", parent.log_id());
    }
    let analysis = actives
        .into_iter()
        .next()
        .expect("a non-empty vector to yield an element");
    Ok(Context { parent, analysis })
}

fn fetch_active(
    conn: &mut PgConnection,
    node: &PrefixTree,
) -> Result<Vec<SplitAnalysis>, ContextFetchError> {
    use crate::schema::split_analysis::dsl::*;

    split_analysis
        .filter(tree_net.eq6(&node.net))
        .filter(result.is_null())
        .order_by(created_at.desc())
        .load(conn)
        .fix_cause()
        .map_err(ContextFetchError::DbError)
}

pub fn fetch_by_follow_up(conn: &mut PgConnection, request_id: &TraceRequestId) -> ContextFetchResult {
    let target_net = find_follow_up_prefix(conn, request_id)?;
    let parent = prefix_tree::context::fetch(conn, &target_net)
        .map_err(ContextFetchError::DbError)?;
    fetch(conn, parent)
}

fn find_follow_up_prefix(
    conn: &mut PgConnection,
    id: &TraceRequestId,
) -> Result<Ipv6Net, ContextFetchError> {
    use crate::schema::split_analysis::dsl as analysis_dsl;
    use crate::schema::prefix_tree::dsl::*;
    let nets: Vec<IpNet> = prefix_tree.inner_join(analysis_dsl::split_analysis)
        .filter(analysis_dsl::pending_follow_up.eq(id.to_string()))
        .filter(net.eq(analysis_dsl::tree_net))
        .select(net)
        .load(conn)
        .fix_cause()
        .map_err(ContextFetchError::DbError)?;
    match *nets.as_slice() {
        [only] => Ok(must_v6(only)),
        [] => Err(ContextFetchError::NoMatchingAnalysis { id: *id }),
        [first, ..] => {
            warn!("Multiple analyses are waiting for follow-up {}", id);
            Ok(must_v6(first))
        }
    }
}

fn must_v6(net: IpNet) -> Ipv6Net {
    match net {
        IpNet::V4(net) => panic!("Unexpected IPv4 net {}", net),
        IpNet::V6(net) => net,
    }
}
