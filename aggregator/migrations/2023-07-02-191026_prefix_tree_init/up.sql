CREATE TYPE prefix_merge_status AS ENUM ('not_merged');

CREATE TABLE public.prefix_tree
(
    id           bigserial PRIMARY KEY NOT NULL,
    "path"       cidr                  NOT NULL,
    created_at   timestamp             NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at   timestamp             NOT NULL DEFAULT CURRENT_TIMESTAMP,
    is_routed    bool                  NOT NULL DEFAULT true,
    merge_status prefix_merge_status   NOT NULL DEFAULT 'not_merged',
    "data"       jsonb                 NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX IF NOT EXISTS prefix_tree_gist_idx ON public.prefix_tree USING gist (path inet_ops);

ALTER TABLE prefix_tree
    ADD CONSTRAINT prefix_tree_path_uq UNIQUE ("path");

SELECT diesel_manage_updated_at('prefix_tree');
