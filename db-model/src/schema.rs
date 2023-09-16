// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "prefix_merge_status"))]
    pub struct PrefixMergeStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "prefix_priority_class"))]
    pub struct PrefixPriorityClass;
}

diesel::table! {
    as_prefix (net) {
        net -> Cidr,
        deleted -> Bool,
        asn -> Int8,
    }
}

diesel::table! {
    measurement_tree (target_net) {
        target_net -> Cidr,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        responsive_count -> Int4,
        unresponsive_count -> Int4,
        last_hop_routers -> Jsonb,
        weirdness -> Jsonb,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::PrefixMergeStatus;
    use super::sql_types::PrefixPriorityClass;

    prefix_tree (net) {
        net -> Cidr,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        is_routed -> Bool,
        merge_status -> PrefixMergeStatus,
        priority_class -> PrefixPriorityClass,
        confidence -> Int2,
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
    split_analysis (id) {
        id -> Int8,
        tree_net -> Cidr,
        created_at -> Timestamp,
        completed_at -> Nullable<Timestamp>,
        #[max_length = 34]
        pending_follow_up -> Nullable<Bpchar>,
        result -> Nullable<Jsonb>,
    }
}

diesel::joinable!(split_analysis -> prefix_tree (tree_net));

diesel::allow_tables_to_appear_in_same_query!(
    as_prefix,
    measurement_tree,
    prefix_tree,
    response_archive,
    split_analysis,
);
