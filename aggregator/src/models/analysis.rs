use chrono::NaiveDateTime;
pub use data::*;
use diesel::prelude::*;

use super::tree::PrefixTree;

#[derive(diesel_derive_enum::DbEnum, Debug, Copy, Clone, PartialEq, Eq)]
#[ExistingTypePath = "crate::schema::sql_types::SplitAnalysisStage"]
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

#[derive(Queryable, Selectable, Identifiable, Associations, Debug, Clone)]
#[diesel(table_name = crate::schema::split_analysis_split)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(analysis_id, split_num))]
#[diesel(belongs_to(SplitAnalysis, foreign_key = analysis_id))]
pub struct Split {
    pub analysis_id: i64,
    pub split_num: i16,
    pub data: SplitData,
}

#[derive(Debug)]
pub struct SplitAnalysisDetails {
    pub analysis: SplitAnalysis,
    pub splits: Vec<Split>,
}

mod data {
    use std::collections::HashSet;
    use std::net::Ipv6Addr;

    use diesel;
    use diesel::backend::Backend;
    use diesel::deserialize::{FromSql, FromSqlRow};
    use diesel::expression::AsExpression;
    use diesel::pg::Pg;
    use diesel::serialize::{Output, ToSql};
    use diesel::sql_types::Jsonb;
    use serde::{Deserialize, Serialize};
    use serde_json;

    #[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
    pub enum LastHopRouterSource {
        TraceUnresponsive,
        TraceResponsive,
        DestUnreachProhibit, // admin-prohibit, failed-egress
        DestUnreachAddrPort, // addr/port unreach
        DestUnreachReject,   // reject-route
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub struct LastHopRouterData {
        pub address: Ipv6Addr,
        pub source: LastHopRouterSource,
        pub hits: i32,
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    pub enum FollowUp {
        TraceResponsive {
            targets: Vec<Ipv6Addr>,
            sent_ttl: u8,
        },
        TraceUnresponsive {
            candidates: Vec<Ipv6Addr>,
        },
    }

    #[derive(FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Default, Clone)]
    #[diesel(sql_type = Jsonb)]
    pub struct SplitData {
        // IMPORTANT: Type must stay backwards-compatible with previously-written JSON,
        // i.e. add only optional fields or provide defaults!
        #[serde(default)]
        pub last_hop_routers: Vec<LastHopRouterData>,

        #[serde(default)]
        pub pending_follow_ups: Vec<FollowUp>,

        #[serde(default)]
        pub weird_behaviours: HashSet<String>,
    }

    impl FromSql<Jsonb, Pg> for SplitData {
        fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            // NOTE: Diesel intentionally doesn't provide this implementation, as it may
            // fail if invalid/unexpected data is stored in the DB... We need to be extra careful.

            let value = <serde_json::Value as FromSql<Jsonb, Pg>>::from_sql(bytes)?;
            Ok(serde_json::from_value(value)?)
        }
    }

    impl ToSql<Jsonb, Pg> for SplitData {
        fn to_sql(&self, out: &mut Output<Pg>) -> diesel::serialize::Result {
            let value = serde_json::to_value(self)?;
            // We need reborrow() to reduce the lifetime of &mut out; mustn't outlive `value`
            <serde_json::Value as ToSql<Jsonb, Pg>>::to_sql(&value, &mut out.reborrow())
        }
    }
}
