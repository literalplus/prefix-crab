use diesel::prelude::*;
use log::warn;
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
