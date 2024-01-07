use chrono::NaiveDateTime;
use diesel;
use diesel::pg::Pg;
use diesel::sql_types::*;
use diesel::{associations::HasTable, prelude::*};

use ipnet::{IpNet, Ipv6Net};
use serde::{Deserialize, Serialize};

use crate::analyse::Confidence;

#[derive(diesel_derive_enum::DbEnum, Debug, Copy, Clone, PartialEq, Eq)]
#[ExistingTypePath = "crate::sql_types::PrefixMergeStatus"]
pub enum MergeStatus {
    Leaf,
    MinSizeReached,
    SplitDown,
    MergedUp,
    UnsplitRoot,
    SplitRoot,
    Blocked, // won't get automatically unblocked, leaf/root state doesn't matter
}

impl MergeStatus {
    pub fn is_eligible_for_split(&self) -> bool {
        matches!(self, MergeStatus::Leaf | MergeStatus::UnsplitRoot)
    }

    pub fn split(&self) -> MergeStatus {
        if matches!(self, MergeStatus::UnsplitRoot | MergeStatus::SplitRoot) {
            MergeStatus::SplitRoot
        } else if self == &MergeStatus::Blocked {
            MergeStatus::Blocked
        } else {
            MergeStatus::SplitDown
        }
    }

    pub fn new(child_prefix_len: u8) -> MergeStatus {
        if child_prefix_len >= 64 {
            MergeStatus::MinSizeReached
        } else {
            MergeStatus::Leaf
        }
    }
}

#[derive(
    Default,
    diesel_derive_enum::DbEnum,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    Copy,
    Clone,
    Hash,
    Ord,
    PartialOrd,
)]
#[ExistingTypePath = "crate::sql_types::PrefixPriorityClass"]
pub enum PriorityClass {
    // Important: Used in the database, do not change incompatibly!

    #[default]
    HighFresh,
    HighOverlapping,
    HighDisjoint,
    // same LHR set with multiple members (but different ratio)
    MediumSameMulti,
    // same LHR set with multiple members and same ratio (within reasonable margin)
    MediumSameRatio,
    // same single LHR
    MediumSameSingle,
    MediumMultiWeird,
    LowWeird,
    LowUnknown,
}

#[derive(Queryable, Debug, Copy, Clone, PartialEq, Eq)]
#[diesel(table_name = crate::schema::prefix_tree)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(net))]
pub struct PrefixTree {
    #[diesel(deserialize_as = crate::persist::Ipv6NetLoader)]
    pub net: Ipv6Net,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub merge_status: MergeStatus,
    pub priority_class: PriorityClass,
    #[diesel(deserialize_as = crate::persist::ConfidenceLoader)]
    pub confidence: Confidence,
}

impl HasTable for PrefixTree {
    type Table = crate::schema::prefix_tree::table;

    fn table() -> Self::Table {
        crate::schema::prefix_tree::table
    }
}

impl<'a> Identifiable for &'a PrefixTree {
    type Id = IpNet;

    fn id(self) -> Self::Id {
        IpNet::V6(self.net)
    }
}

use crate::schema::prefix_tree::dsl;
impl Selectable<Pg> for PrefixTree {
    type SelectExpression = (
        dsl::net,
        dsl::created_at,
        dsl::updated_at,
        dsl::merge_status,
        dsl::priority_class,
        dsl::confidence,
    );

    fn construct_selection() -> Self::SelectExpression {
        use dsl::*;
        (
            net,
            created_at,
            updated_at,
            merge_status,
            priority_class,
            confidence,
        )
    }
}

#[derive(Queryable, Debug, Copy, Clone)]
#[diesel(table_name = crate::schema::as_prefix)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(net))]
pub struct AsPrefix {
    #[diesel(deserialize_as = crate::persist::Ipv6NetLoader)]
    pub net: Ipv6Net,
    pub deleted: bool,
    pub asn: i64,
}
