CREATE EXTENSION IF NOT EXISTS "ltree";

CREATE TYPE prefix_merge_status AS ENUM ('none');

CREATE TABLE public.prefix_tree
(
    id           bigserial PRIMARY KEY NOT NULL,
    "path"       public.ltree          NOT NULL,
    created      timestamp             NOT NULL DEFAULT CURRENT_TIMESTAMP,
    modified     timestamp             NOT NULL DEFAULT CURRENT_TIMESTAMP,
    is_routed    bool                  NOT NULL DEFAULT true,
    merge_status prefix_merge_status   NOT NULL DEFAULT 'none',
    "data"       jsonb                 NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX IF NOT EXISTS prefix_tree_gist_idx ON public.prefix_tree USING gist (path);
