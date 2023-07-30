// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "prefix_merge_status"))]
    pub struct PrefixMergeStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "split_analysis_stage"))]
    pub struct SplitAnalysisStage;
}

diesel::table! {
    measurement_tree (target_net) {
        target_net -> Cidr,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        hit_count -> Int4,
        miss_count -> Int4,
        last_hop_routers -> Jsonb,
        weirdness -> Jsonb,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::PrefixMergeStatus;

    prefix_tree (id) {
        id -> Int8,
        path -> Cidr,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        is_routed -> Bool,
        merge_status -> PrefixMergeStatus,
        data -> Jsonb,
    }
}

diesel::table! {
    response_archive (id) {
        id -> Int8,
        path -> Cidr,
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
        #[max_length = 30]
        pending_follow_up -> Nullable<Bpchar>,
    }
}

diesel::joinable!(split_analysis -> prefix_tree (tree_id));

diesel::allow_tables_to_appear_in_same_query!(
    measurement_tree,
    prefix_tree,
    response_archive,
    split_analysis,
);
