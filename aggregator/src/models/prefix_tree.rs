use chrono::NaiveDateTime;
use diesel;
use diesel::prelude::*;

mod data;
mod path;

pub use path::PrefixPath;
pub use data::ExtraData;

#[derive(diesel_derive_enum::DbEnum, Debug)]
#[ExistingTypePath = "crate::schema::sql_types::PrefixMergeStatus"]
pub enum MergeStatus {
    NotMerged,
}

#[derive(Queryable, Selectable)]
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
