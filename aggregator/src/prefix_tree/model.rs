use chrono::NaiveDateTime;
use diesel;
use diesel::pg::Pg;
use diesel::sql_types::*;
use diesel::{associations::HasTable, prelude::*};

use ipnet::{IpNet, Ipv6Net};
use serde::{Deserialize, Serialize};

use crate::analyse::split::Confidence;

#[derive(diesel_derive_enum::DbEnum, Debug, Copy, Clone, PartialEq, Eq)]
#[ExistingTypePath = "crate::sql_types::PrefixMergeStatus"]
pub enum MergeStatus {
    Leaf,
    SplitDown,
    MergedUp,
}

#[derive(
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
    HighFresh,
    HighOverlapping,
    HighDisjoint,
    MediumSameMulti,
    MediumSameSingle,
    MediumMultiWeird,
    LowWeird,
    LowUnknown,
}

impl Default for PriorityClass {
    fn default() -> Self {
        PriorityClass::HighFresh
    }
}

#[derive(Queryable, Debug, Copy, Clone)]
#[diesel(table_name = crate::schema::prefix_tree)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(net))]
pub struct PrefixTree {
    #[diesel(deserialize_as = crate::persist::Ipv6NetLoader)]
    pub net: Ipv6Net,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub is_routed: bool,
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
        dsl::is_routed,
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
            is_routed,
            merge_status,
            priority_class,
            confidence,
        )
    }
}
