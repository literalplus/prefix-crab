use anyhow::{bail, Result};
use chrono::NaiveDateTime;
use diesel;
use diesel::prelude::*;

use ipnet::{IpNet, Ipv6Net};
use serde::{Deserialize, Serialize};

use crate::analyse::HitCount;

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

#[derive(Queryable, Selectable, Identifiable, Debug, Copy, Clone)]
#[diesel(table_name = crate::schema::prefix_tree)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(net))]
pub struct PrefixTree {
    pub net: IpNet,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub is_routed: bool,
    pub merge_status: MergeStatus,
    pub priority_class: PriorityClass,
    pub supporting_evidence: HitCount,
}

impl PrefixTree {
    pub fn try_net_into_v6(&self) -> Result<Ipv6Net> {
        match &self.net {
            IpNet::V4(net) => bail!("encountered prefix tree with IPv4, which is super illegal: {}", net),
            IpNet::V6(net) => Ok(*net),
        }
    }
}
