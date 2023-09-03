CREATE TABLE split_analysis(
    id bigserial PRIMARY KEY NOT NULL,
    -- deleting a prefix with stored analyses should be a conscious decision v
    tree_net cidr NOT NULL REFERENCES prefix_tree(net) ON DELETE RESTRICT,
    created_at timestamp NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'UTC'),
    completed_at timestamp NULL DEFAULT NULL,
    result jsonb NULL DEFAULT NULL
);

CREATE INDEX split_analysis_tree_id_idx ON split_analysis(tree_net);

CREATE TABLE split_analysis_follow_up(
    analysis_id bigint NOT NULL REFERENCES split_analysis(id) ON DELETE CASCADE,
    -- https://github.com/jetpack-io/typeid
    -- 'tracerq_' + 26 character UUID
    follow_up_id character(34) NOT NULL,
    PRIMARY KEY (analysis_id, follow_up_id)
);

CREATE INDEX split_analysis_follow_up_follow_up_id ON split_analysis_follow_up(follow_up_id);
