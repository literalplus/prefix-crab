// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "ltree"))]
    pub struct Ltree;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "prefix_merge_status"))]
    pub struct PrefixMergeStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "split_analysis_stage"))]
    pub struct SplitAnalysisStage;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::Ltree;
    use super::sql_types::PrefixMergeStatus;

    prefix_tree (id) {
        id -> Int8,
        path -> Ltree,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        is_routed -> Bool,
        merge_status -> PrefixMergeStatus,
        data -> Jsonb,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::Ltree;

    response_archive (id) {
        id -> Int8,
        path -> Ltree,
        created_at -> Timestamp,
        data -> Jsonb,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::SplitAnalysisStage;

    split_analysis (id) {
        id -> Int8,
        tree_id -> Int8,
        created_at -> Timestamp,
        completed_at -> Nullable<Timestamp>,
        stage -> SplitAnalysisStage,
        split_prefix_len -> Int2,
    }
}

diesel::table! {
    split_analysis_split (analysis_id, split_num) {
        analysis_id -> Int8,
        split_num -> Int2,
        data -> Jsonb,
    }
}

diesel::joinable!(split_analysis -> prefix_tree (tree_id));
diesel::joinable!(split_analysis_split -> split_analysis (analysis_id));

diesel::allow_tables_to_appear_in_same_query!(
    prefix_tree,
    response_archive,
    split_analysis,
    split_analysis_split,
);
