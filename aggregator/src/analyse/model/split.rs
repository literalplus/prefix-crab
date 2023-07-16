pub use data::*;
use diesel::prelude::*;
use prefix_crab::prefix_split::NetIndex;

use super::SplitAnalysis;

#[derive(Queryable, Selectable, Insertable, Identifiable, Associations, Debug, Clone)]
#[diesel(table_name = crate::persist::schema::split_analysis_split)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(analysis_id, net_index))]
#[diesel(belongs_to(SplitAnalysis, foreign_key = analysis_id))]
pub struct Split {
    pub analysis_id: i64,
    pub net_index: i16,
    pub data: SplitData,
}

impl Split {
    pub fn new(parent: &SplitAnalysis, net_index: NetIndex) -> Self {
        Self {
            analysis_id: parent.id,
            net_index: <u8>::from(net_index) as i16,
            data: SplitData::default(),
        }
    }
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

    use crate::analyse::CanFollowUp;

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

    impl CanFollowUp for SplitData {
        fn needs_follow_up(&self) -> bool {
            !self.pending_follow_ups.is_empty()
        }
    }
}
