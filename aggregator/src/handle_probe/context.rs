use anyhow::*;
use diesel::dsl::not;
use diesel::insert_into;
use diesel::prelude::*;

use crate::models::path::{PathExpressionMethods, PrefixPath};
use crate::models::tree::*;
use crate::schema::prefix_tree::dsl::*;

pub use analyses::ContextAnalyses;

#[derive(Debug)]
pub struct ProbeContext {
    pub node: PrefixTree,
    pub ancestors: Vec<PrefixTree>,
    pub unmerged_children: Vec<PrefixTree>,
    pub analyses: ContextAnalyses,
}

impl ProbeContext {
    pub fn log_id(&self) -> String {
        self.node.path.to_string()
    }

    pub fn refresh_analyses(&mut self, conn: &mut PgConnection) -> Result<()> {
        self.analyses = analyses::fetch(conn, &self.node)?;
        Ok(())
    }
}

pub fn fetch(conn: &mut PgConnection, target_net: &PrefixPath) -> Result<ProbeContext> {
    insert_if_new(conn, target_net)?;

    let ancestors_and_self = select_ancestors_and_self(conn, target_net)
        .with_context(|| "while finding ancestors and self")?;
    let (ancestors, node) = match &ancestors_and_self[..] {
        [parents @ .., node] => (parents.to_vec(), *node),
        [] => bail!("Didn't find the prefix_tree node we just inserted :("),
    };
    Result::Ok(ProbeContext {
        node,
        ancestors,
        unmerged_children: select_unmerged_children(conn, &node)?,
        analyses: analyses::fetch(conn, &node)?,
    })
}

fn select_ancestors_and_self(
    connection: &mut PgConnection,
    target_net: &PrefixPath,
) -> Result<Vec<PrefixTree>> {
    let parents = prefix_tree
        .filter(path.ancestor_or_same_as(target_net))
        .select(PrefixTree::as_select())
        .order_by(path)
        .load(connection)
        .with_context(|| "while selecting parents")?;
    Ok(parents)
}

fn select_unmerged_children(
    connection: &mut PgConnection,
    node: &PrefixTree,
) -> Result<Vec<PrefixTree>> {
    let parents = prefix_tree
        .filter(path.descendant_or_same_as(&node.path))
        .filter(not(path.eq(&node.path)))
        .select(PrefixTree::as_select())
        .order_by(path)
        .load(connection)
        .with_context(|| "while selecting unmerged children")?;
    Ok(parents)
}

fn insert_if_new(connection: &mut PgConnection, target_net: &PrefixPath) -> Result<()> {
    insert_into(prefix_tree)
        .values((
            path.eq(target_net),
            is_routed.eq(true),
            merge_status.eq(MergeStatus::NotMerged),
            data.eq(ExtraData {
                ever_responded: true,
            }),
        ))
        .on_conflict_do_nothing()
        .returning(id)
        .execute(connection)
        .with_context(|| "while trying to insert into prefix_tree")?;
    Ok(())
}

mod analyses {
    use anyhow::*;
    

    use diesel::prelude::*;

    use crate::models::analysis::Split;
    use crate::models::analysis::SplitAnalysis;
    use crate::models::analysis::SplitAnalysisDetails;
    use crate::models::analysis::Stage;
    
    use crate::models::tree::*;
    use crate::schema::split_analysis::dsl::created_at;
    use crate::schema::split_analysis_split::dsl::*;

    #[derive(Debug)]
    pub struct ContextAnalyses {
        pub completed: Vec<SplitAnalysis>,
        pub active: Option<SplitAnalysisDetails>,
    }

    pub fn fetch(conn: &mut PgConnection, node: &PrefixTree) -> Result<ContextAnalyses> {
        let all = fetch_all(conn, node)?;
        let completed = all
            .iter()
            .filter(|it| it.stage == Stage::Completed)
            .cloned()
            .collect();
        let most_recent_and_active = all.into_iter()
        .filter(|it| it.stage != Stage::Completed)
        .next();
        let active = if let Some(analysis) = most_recent_and_active {
            Some(fetch_details(conn, analysis)?)
        } else {
            None
        };
        Ok(ContextAnalyses {
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
        Ok(SplitAnalysisDetails::new( analysis, splits))
    }
}
