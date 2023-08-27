use chrono::NaiveDateTime;
use diesel::{prelude::*, associations::HasTable};
use diesel;

use ipnet::{Ipv6Net, IpNet};
use serde::{Deserialize, Serialize};

use crate::analyse::split::Confidence;


#[derive(diesel_derive_enum::DbEnum, Debug, Copy, Clone)]
#[ExistingTypePath = "crate::sql_types::PrefixMergeStatus"]
pub enum MergeStatus {
    Leaf,
    SplitDown,
    MergedUp,
}

#[derive(diesel_derive_enum::DbEnum, Debug, Eq, PartialEq, Serialize, Deserialize, Copy, Clone)]
#[ExistingTypePath = "crate::sql_types::PrefixPriorityClass"]
pub enum PriorityClass {
    // Important: Used in the database, do not change incompatibly!
    LowUnknown,
    LowWeird,
    MediumMultiWeird,
    MediumSameSingle,
    MediumSameMulti,
    HighDisjoint,
    HighOverlapping,
    HighFresh,
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
