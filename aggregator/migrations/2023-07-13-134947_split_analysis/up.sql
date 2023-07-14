CREATE TYPE split_analysis_stage AS ENUM(
    'requested',
    'pending_trace',
    'completed'
);

CREATE TABLE split_analysis(
    id bigserial PRIMARY KEY NOT NULL,
    -- deleting a prefix with stored analyses should be a conscious decision v
    tree_id bigint NOT NULL REFERENCES prefix_tree(id) ON DELETE RESTRICT,
    created_at timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at timestamp NULL DEFAULT NULL,
    stage split_analysis_stage NOT NULL DEFAULT 'requested',
    split_prefix_len smallint NOT NULL
);

CREATE INDEX split_analysis_tree_id_idx ON split_analysis(tree_id);

CREATE TABLE split_analysis_split(
    analysis_id bigint NOT NULL REFERENCES split_analysis(id) ON DELETE CASCADE,
    split_num smallint NOT NULL,
    data jsonb NOT NULL DEFAULT '{}' ::jsonb,
    PRIMARY KEY (analysis_id, split_num)
);

CREATE INDEX split_analysis_split_analysis_id_idx ON split_analysis_split(analysis_id);

-- could cluster the table on tree_id for select performance, but that would require a periodic
-- re-cluster job to stay effective with new data. one way to handle this could be pgagent,
-- provided by pgadmin (or a sufficiently-advanced cronjob).
