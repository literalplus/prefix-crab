macro_rules! configure_jsonb_serde {
    ($type:ty) => {
        impl diesel::deserialize::FromSql<diesel::sql_types::Jsonb, diesel::pg::Pg> for $type {
            fn from_sql(
                bytes: <diesel::pg::Pg as diesel::backend::Backend>::RawValue<'_>,
            ) -> diesel::deserialize::Result<Self> {
                // NOTE: Diesel intentionally doesn't provide this implementation, as it may
                // fail if invalid/unexpected data is stored in the DB... We need to be extra careful.
                use anyhow::Context;
                use diesel::{deserialize::FromSql, pg::Pg, sql_types::Jsonb};
                use serde_json::{from_value, Value};

                let parsed_json = <Value as FromSql<Jsonb, Pg>>::from_sql(bytes)?;
                let typed_json = from_value(parsed_json).map_err(anyhow::Error::from);
                Ok(typed_json.context("converting JSON into $type")?)
            }
        }

        impl diesel::serialize::ToSql<diesel::sql_types::Jsonb, diesel::pg::Pg> for $type {
            fn to_sql(
                &self,
                out: &mut diesel::serialize::Output<diesel::pg::Pg>,
            ) -> diesel::serialize::Result {
                use anyhow::Context as _;
                use diesel::{pg::Pg, serialize::ToSql, sql_types::Jsonb};
                use serde_json::{to_value, Value};

                let parsed_json2 = to_value(self).context("serializing $type")?;
                // We need reborrow() to reduce the lifetime of &mut out; mustn't outlive `value`
                <Value as ToSql<Jsonb, Pg>>::to_sql(&parsed_json2, &mut out.reborrow())
            }
        }
    };
}

pub(crate) use configure_jsonb_serde;
use diesel::{QueryResult, result::Error as DieselError};
use anyhow::anyhow;

pub trait DieselErrorFixCause<T> {
    /// Diesel's error currently breaks cause chains to wrapped errors. Without special treatmeant, any details
    /// of inner errors (e.g. from serde_json) are lost and not displayed in backtraces or error prints.
    /// 
    /// This fixes the issue by unwrapping the inner errors.
    fn fix_cause(self) -> anyhow::Result<T>;
}

impl<T> DieselErrorFixCause<T> for QueryResult<T> {
    fn fix_cause(self) -> anyhow::Result<T> {
        self.map_err(unwrap_diesel_err)
    }
}

fn unwrap_diesel_err(diesel_err: DieselError) -> anyhow::Error {
    match diesel_err {
        // Diesel's error doesn't implement the new source() yet, so we un-break the source() chain manually
        // This match is similar to their impl of the deprecated cause(), which the borrow checker doesn't like
        diesel::result::Error::DeserializationError(e) => anyhow!(e),
        diesel::result::Error::SerializationError(e) => anyhow!(e),
        diesel::result::Error::QueryBuilderError(e) => anyhow!(e),
        diesel::result::Error::InvalidCString(e) => anyhow!(e),
        e => anyhow!(e),
    }
}
