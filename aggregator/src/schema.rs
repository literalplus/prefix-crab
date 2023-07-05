// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "ltree"))]
    pub struct Ltree;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "prefix_merge_status"))]
    pub struct PrefixMergeStatus;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::Ltree;
    use super::sql_types::PrefixMergeStatus;

    prefix_tree (id) {
        id -> Int8,
        path -> Ltree,
        created -> Timestamp,
        modified -> Timestamp,
        is_routed -> Bool,
        merge_status -> PrefixMergeStatus,
        data -> Jsonb,
    }
}
