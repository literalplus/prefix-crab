CREATE TYPE prefix_merge_status AS ENUM (
    'leaf', 'split_down', 'merged_up', 'unsplit_root', 'split_root'
);
CREATE TYPE prefix_priority_class AS ENUM(
    'low_unknown', 'low_weird',
    'medium_multi_weird', 'medium_same_single', 'medium_same_multi',
    'high_disjoint', 'high_overlapping', 'high_fresh'
);

CREATE TABLE public.prefix_tree
(
    net                 cidr PRIMARY KEY      NOT NULL,
    created_at          timestamp             NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'UTC'),
    updated_at          timestamp             NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'UTC'),
    merge_status        prefix_merge_status   NOT NULL DEFAULT 'leaf',
    priority_class      prefix_priority_class NOT NULL DEFAULT 'high_fresh',
    confidence          smallint              NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS prefix_tree_gist_idx ON public.prefix_tree USING gist (net inet_ops);

-- speed up search for prefixes that can be operated on
CREATE INDEX IF NOT EXISTS prefix_tree_merge_status_idx ON public.prefix_tree (merge_status)
    WHERE merge_status IN ('leaf', 'unsplit_root');

SELECT diesel_manage_updated_at('prefix_tree');
