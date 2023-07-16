use chrono::NaiveDateTime;
use diesel::prelude::*;

use crate::prefix_tree::PrefixTree;

#[derive(diesel_derive_enum::DbEnum, Debug, Copy, Clone, PartialEq, Eq)]
#[ExistingTypePath = "crate::sql_types::SplitAnalysisStage"]
pub enum Stage {
    Requested,
    PendingTrace,
    Completed,
}

#[derive(Queryable, Selectable, Identifiable, Associations, Debug, Copy, Clone)]
#[diesel(table_name = crate::schema::split_analysis)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(belongs_to(PrefixTree, foreign_key = tree_id))]
pub struct SplitAnalysis {
    pub id: i64,
    pub tree_id: i64,
    pub created_at: NaiveDateTime,
    pub completed_at: Option<NaiveDateTime>,
    pub stage: Stage,
    pub split_prefix_len: i16,
}
