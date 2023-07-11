
CREATE TABLE public.response_archive
(
    id           bigserial PRIMARY KEY NOT NULL,
    "path"       public.ltree          NOT NULL,
    created      timestamp             NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "data"       jsonb                 NOT NULL DEFAULT '{}'::jsonb
);
