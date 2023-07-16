use anyhow::*;

use diesel::prelude::*;

use crate::prefix_tree::context::ContextOps;
use crate::prefix_tree::{self, PrefixTree};
use crate::schema::split_analysis::dsl::created_at;
use crate::schema::split_analysis_split::dsl::*;

use super::{Split, SplitAnalysis, SplitAnalysisDetails, Stage};

#[derive(Debug)]
pub struct Context {
    pub parent: prefix_tree::Context,
    pub completed: Vec<SplitAnalysis>,
    pub active: Option<SplitAnalysisDetails>,
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
    let most_recent_and_active = all.into_iter().find(|it| it.stage != Stage::Completed);
    let active = if let Some(analysis) = most_recent_and_active {
        Some(fetch_details(conn, analysis)?)
    } else {
        None
    };
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

fn fetch_details(conn: &mut PgConnection, analysis: SplitAnalysis) -> Result<SplitAnalysisDetails> {
    let splits = Split::belonging_to(&analysis)
        .select(Split::as_select())
        .order_by(net_index)
        .load(conn)?;
    Ok(SplitAnalysisDetails::new(analysis, splits))
}
