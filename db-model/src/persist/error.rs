use anyhow::anyhow;
use diesel::{result::Error as DieselError, QueryResult};

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
