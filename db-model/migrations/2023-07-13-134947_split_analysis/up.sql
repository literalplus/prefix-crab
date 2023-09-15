CREATE TABLE split_analysis(
    id bigserial PRIMARY KEY NOT NULL,
    -- deleting a prefix with stored analyses should be a conscious decision v
    tree_net cidr NOT NULL REFERENCES prefix_tree(net) ON DELETE RESTRICT,
    created_at timestamp NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'UTC'),
    completed_at timestamp NULL DEFAULT NULL,
    -- https://github.com/jetpack-io/typeid
    -- 'tracerq_' + 26 character UUID
    pending_follow_up character(34) NULL DEFAULT NULL,
    result jsonb NULL DEFAULT NULL
);

CREATE INDEX split_analysis_tree_id_idx ON split_analysis(tree_net);
CREATE INDEX split_analysis_pending_follow_up_idx ON split_analysis(pending_follow_up);
