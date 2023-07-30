use anyhow::*;

use diesel::prelude::*;

use crate::prefix_tree::context::ContextOps;
use crate::prefix_tree::{self, PrefixTree};
use crate::schema::split_analysis::dsl::created_at;

use super::{SplitAnalysis, Stage};

#[derive(Debug)]
pub struct Context {
    pub parent: prefix_tree::Context,
    pub completed: Vec<SplitAnalysis>,
    pub active: Option<SplitAnalysis>,
}

impl ContextOps for Context {
    fn log_id(&self) -> String {
        self.parent.log_id()
    }

    fn node(&self) -> &PrefixTree {
        &self.parent.node
    }
}

pub fn fetch(conn: &mut PgConnection, parent: prefix_tree::Context) -> Result<Context> {
    let all = fetch_all(conn, &parent.node)?;
    let completed = all
        .iter()
        .filter(|it| it.stage == Stage::Completed)
        .cloned()
        .collect();
    let active = all.into_iter().find(|it| it.stage != Stage::Completed);
    Ok(Context {
        parent,
        completed,
        active,
    })
}

fn fetch_all(conn: &mut PgConnection, node: &PrefixTree) -> Result<Vec<SplitAnalysis>> {
    // TODO ignore very old analyses or cleanup ?
    Ok(SplitAnalysis::belonging_to(node)
        .select(SplitAnalysis::as_select())
        .order_by(created_at.desc())
        .load(conn)?)
}
