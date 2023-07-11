use chrono::NaiveDateTime;
use diesel;
use diesel::prelude::*;

pub use data::ExtraData;
use super::path::PrefixPath;

#[derive(diesel_derive_enum::DbEnum, Debug, Copy, Clone)]
#[ExistingTypePath = "crate::schema::sql_types::PrefixMergeStatus"]
pub enum MergeStatus {
    NotMerged,
}

#[derive(Queryable, Selectable, Debug, Copy, Clone)]
#[diesel(table_name = crate::schema::prefix_tree)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PrefixTree {
    pub id: i64,
    pub path: PrefixPath,
    pub created: NaiveDateTime,
    pub modified: NaiveDateTime,
    pub is_routed: bool,
    pub merge_status: MergeStatus,
    pub data: ExtraData,
}

mod data {
    use diesel;
    use diesel::backend::Backend;
    use diesel::deserialize::{FromSql, FromSqlRow};
    use diesel::expression::AsExpression;
    use diesel::pg::Pg;
    use diesel::serialize::{Output, ToSql};
    use diesel::sql_types::Jsonb;
    use serde::{Deserialize, Serialize};
    use serde_json;

    #[derive(FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Default, Copy, Clone)]
    #[diesel(sql_type = Jsonb)]
    pub struct ExtraData {
        // IMPORTANT: Type must stay backwards-compatible with previously-written JSON,
        // i.e. add only optional fields or provide defaults!

        pub ever_responded: bool,
    }

    impl FromSql<Jsonb, Pg> for ExtraData {
        fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            // NOTE: Diesel intentionally doesn't provide this implementation, as it may
            // fail if invalid/unexpected data is stored in the DB... We need to be extra careful.

            let value = <serde_json::Value as FromSql<Jsonb, Pg>>::from_sql(bytes)?;
            Ok(serde_json::from_value(value)?)
        }
    }

    impl ToSql<Jsonb, Pg> for ExtraData {
        fn to_sql(&self, out: &mut Output<Pg>) -> diesel::serialize::Result {
            let value = serde_json::to_value(self)?;
            // We need reborrow() to reduce the lifetime of &mut out; mustn't outlive `value`
            <serde_json::Value as ToSql<Jsonb, Pg>>::to_sql(&value, &mut out.reborrow())
        }
    }
}