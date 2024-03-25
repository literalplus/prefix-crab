use chrono::NaiveDateTime;
use diesel::prelude::*;
use ipnet::Ipv6Net;
use serde::{Deserialize, Serialize};

use crate::{
    persist::configure_jsonb_serde,
    prefix_tree::{PrefixTree, PriorityClass},
};

use super::HitCount;

#[derive(Queryable, Identifiable, Associations, PartialEq, Debug, Clone)]
#[diesel(table_name = crate::schema::split_analysis)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(belongs_to(PrefixTree, foreign_key = tree_net))]
pub struct SplitAnalysis {
    // NOTE: `belonging_to` doesn't work for this struct since the parent doesn't return an &IpNet for its ID
    // - that is not possible because we construct it directly in the function as a temporary value.
    pub id: i64,
    #[diesel(deserialize_as = crate::persist::Ipv6NetLoader)]
    pub tree_net: Ipv6Net,
    pub created_at: NaiveDateTime,
    pub completed_at: Option<NaiveDateTime>,
    pub pending_follow_up: Option<String>, // Actually TraceRequestId
    pub result: Option<SplitAnalysisResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SplitAnalysisResult {
    // Important: JSONB field, must stay compatible!
    pub class: PriorityClass,
    pub evidence: HitCount,
    pub should_split: Option<bool>, // missing = we don't know
    pub algo_version: i32,
}

configure_jsonb_serde!(SplitAnalysisResult);
