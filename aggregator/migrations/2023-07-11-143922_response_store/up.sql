
CREATE TABLE public.response_archive
(
    id           bigserial PRIMARY KEY NOT NULL,
    "path"       cidr                  NOT NULL,
    created_at   timestamp             NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'UTC'),
    "data"       jsonb                 NOT NULL DEFAULT '{}'::jsonb
);
