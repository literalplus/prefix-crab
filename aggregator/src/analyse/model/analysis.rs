use chrono::NaiveDateTime;
use diesel::prelude::*;
use ipnet::IpNet;
use serde::{Serialize, Deserialize};

use crate::{prefix_tree::{PrefixTree, PriorityClass}, persist::configure_jsonb_serde};

use super::HitCount;

#[derive(Queryable, Selectable, Identifiable, Associations, Debug, Clone)]
#[diesel(table_name = crate::schema::split_analysis)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(belongs_to(PrefixTree, foreign_key = tree_net))]
pub struct SplitAnalysis {
    pub id: i64,
    pub tree_net: IpNet,
    pub created_at: NaiveDateTime,
    pub completed_at: Option<NaiveDateTime>,
    pub pending_follow_up: Option<String>,
    pub result: Option<SplitAnalysisResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SplitAnalysisResult {
    // Important: JSONB field, must stay compatible!

    pub class: PriorityClass,
    pub evidence: HitCount,
}

configure_jsonb_serde!(SplitAnalysisResult);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Stage {
    Requested,
    PendingTrace,
    Completed,
}

impl SplitAnalysis {
    fn stage(&self) -> Stage {
        if self.result.is_some() {
            Stage::Completed
        } else if self.pending_follow_up.is_some() {
            Stage::PendingTrace
        } else {
            Stage::Requested
        }
    }
}