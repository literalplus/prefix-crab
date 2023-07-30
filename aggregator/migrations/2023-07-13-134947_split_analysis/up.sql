CREATE TYPE split_analysis_stage AS ENUM(
    'requested',
    'pending_trace',
    'completed'
);

CREATE TABLE split_analysis(
    id bigserial PRIMARY KEY NOT NULL,
    -- deleting a prefix with stored analyses should be a conscious decision v
    tree_id bigint NOT NULL REFERENCES prefix_tree(id) ON DELETE       RESTRICT,
    created_at timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at timestamp NULL DEFAULT NULL,
    stage split_analysis_stage NOT NULL DEFAULT 'requested',
    -- https://github.com/jetpack-io/typeid
    -- 'fou_' + 26 character UUID
    pending_follow_up character(30) NULL DEFAULT NULL
);

CREATE INDEX split_analysis_tree_id_idx ON split_analysis(tree_id);
