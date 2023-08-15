use anyhow::{bail, Result};
use chrono::NaiveDateTime;
use diesel;
use diesel::prelude::*;

use diesel::deserialize::FromSqlRow;
use diesel::expression::AsExpression;
use diesel::sql_types::Jsonb;
use ipnet::{IpNet, Ipv6Net};
use serde::{Deserialize, Serialize};

#[derive(diesel_derive_enum::DbEnum, Debug, Copy, Clone)]
#[ExistingTypePath = "crate::sql_types::PrefixMergeStatus"]
pub enum MergeStatus {
    NotMerged,
}

#[derive(Queryable, Selectable, Identifiable, Debug, Copy, Clone)]
#[diesel(table_name = crate::schema::prefix_tree)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PrefixTree {
    pub id: i64,
    pub path: IpNet,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub is_routed: bool,
    pub merge_status: MergeStatus,
    pub data: ExtraData,
}

impl PrefixTree {
    pub fn try_net_into_v6(&self) -> Result<Ipv6Net> {
        match &self.path {
            IpNet::V4(net) => bail!("encountered prefix tree with IPv4, which is super illegal: {}", net),
            IpNet::V6(net) => Ok(*net),
        }
    }
}

#[derive(FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Default, Copy, Clone)]
#[diesel(sql_type = Jsonb)]
pub struct ExtraData {
    // IMPORTANT: Type must stay backwards-compatible with previously-written JSON,
    // i.e. add only optional fields or provide defaults!
    pub ever_responded: bool,
}

crate::persist::configure_jsonb_serde!(ExtraData);
