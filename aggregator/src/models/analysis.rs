use std::{collections::HashMap, ops::{Index, IndexMut}};

use chrono::NaiveDateTime;
pub use data::*;
use diesel::prelude::*;
use prefix_crab::prefix_split::NetIndex;

use crate::handle_probe::interpret::model::CanFollowUp;

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

#[derive(Queryable, Selectable, Insertable, Identifiable, Associations, Debug, Clone)]
#[diesel(table_name = crate::schema::split_analysis_split)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(analysis_id, net_index))]
#[diesel(belongs_to(SplitAnalysis, foreign_key = analysis_id))]
pub struct Split {
    pub analysis_id: i64,
    pub net_index: i16,
    pub data: SplitData,
}

impl Split {
    fn new(parent: &SplitAnalysis, net_index: NetIndex) -> Self {
        Self {
            analysis_id: parent.id,
            net_index: <u8>::from(net_index) as i16,
            data: SplitData::default(),
        }
    }
}

#[derive(Debug)]
pub struct SplitAnalysisDetails {
    pub analysis: SplitAnalysis,
    splits: Vec<Split>,
}

impl SplitAnalysisDetails {
    pub fn new(analysis: SplitAnalysis, existing_splits: Vec<Split>) -> Self {
        let mut map = HashMap::<NetIndex, Split>::with_capacity(NetIndex::value_count() as usize);
        for existing_split in existing_splits {
            if let Ok(valid_index) = <NetIndex>::try_from(existing_split.net_index) {
                map.insert(valid_index, existing_split);
            }
        }
        for index in NetIndex::iter_values() {
            if !map.contains_key(&index) {
                map.insert(index, Split::new(&analysis, index));
            }
        }
        let splits = map.into_values().collect();
        Self {
            analysis,
            splits,
        }
    }

    pub fn borrow_splits(&self) -> &Vec<Split> {
        &self.splits
    }
}

impl Index<&NetIndex> for SplitAnalysisDetails {
    type Output = Split;

    /// This should never fail under normal circumstances due to pre-filling of
    /// splits up to [NetIndex::count_values] at construction time.
    fn index(&self, index: &NetIndex) -> &Self::Output {
        &self.splits[<usize>::from(*index)]
    }
}

impl IndexMut<&NetIndex> for SplitAnalysisDetails {
    /// This should never fail under normal circumstances due to pre-filling of
    /// splits up to [NetIndex::count_values] at construction time.
    fn index_mut(&mut self, index: &NetIndex) -> &mut Self::Output {
        &mut self.splits[<usize>::from(*index)]
    }
}

impl CanFollowUp for SplitAnalysisDetails {
    fn needs_follow_up(&self) -> bool {
        self.splits.iter().any(|it| it.data.needs_follow_up())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use assertor::*;

    use super::*;

    #[test]
    fn fill_splits() {
        // given
        let analysis = SplitAnalysis {
            id: 0i64,
            tree_id: 0i64,
            created_at: NaiveDateTime::default(),
            completed_at: None,
            stage: Stage::PendingTrace,
            split_prefix_len: 2,
        };
        let my_index = NetIndex::try_from(0u8).unwrap();
        let mut my_split = Split::new(&analysis, *&my_index);
        let my_weirdness = "henlo";
        my_split.data.weird_behaviours.insert(my_weirdness.to_string());
        let existing = vec![my_split];
        // when
        let created = SplitAnalysisDetails::new(analysis, existing);
        // then
        assert_that!(created.splits).has_length(NetIndex::value_count() as usize);
        let mut expected = HashSet::<String>::new();
        expected.insert(my_weirdness.to_string());
        assert_that!(created[&my_index].data.weird_behaviours).is_equal_to(expected);
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

    use crate::handle_probe::interpret::model::CanFollowUp;

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
            return !self.pending_follow_ups.is_empty();
        }
    }
}
