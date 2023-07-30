macro_rules! configure_jsonb_serde {
    ($type:ty) => {
        impl diesel::deserialize::FromSql<diesel::sql_types::Jsonb, diesel::pg::Pg> for $type {
            fn from_sql(bytes: <diesel::pg::Pg as diesel::backend::Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
                // NOTE: Diesel intentionally doesn't provide this implementation, as it may
                // fail if invalid/unexpected data is stored in the DB... We need to be extra careful.
                use anyhow::Context;
                use diesel::{deserialize::FromSql, sql_types::Jsonb, pg::Pg};
                use serde_json::{Value, from_value};

                let parsed_json = <Value as FromSql<Jsonb, Pg>>::from_sql(bytes)?;
                let typed_json = from_value(parsed_json).map_err(anyhow::Error::from);
                Ok(typed_json.context("converting JSON into $type")?)
            }
        }

        impl diesel::serialize::ToSql<diesel::sql_types::Jsonb, diesel::pg::Pg> for $type {
            fn to_sql(&self, out: &mut diesel::serialize::Output<diesel::pg::Pg>) -> diesel::serialize::Result {
                use anyhow::Context as _;
                use diesel::{serialize::ToSql, sql_types::Jsonb, pg::Pg};
                use serde_json::{Value, to_value};

                let parsed_json2 = to_value(self).context("serializing $type")?;
                // We need reborrow() to reduce the lifetime of &mut out; mustn't outlive `value`
                <Value as ToSql<Jsonb, Pg>>::to_sql(&parsed_json2, &mut out.reborrow())
            }
        }
    };
}

pub(crate) use configure_jsonb_serde;
