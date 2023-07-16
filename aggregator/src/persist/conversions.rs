mod path;

mod tree {
    
    use diesel::backend::Backend;
    use diesel::deserialize::FromSql;
    use diesel::pg::Pg;
    use diesel::serialize::{Output, ToSql};
    use diesel::sql_types::Jsonb;

    use crate::prefix_tree::model::ExtraData;

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
