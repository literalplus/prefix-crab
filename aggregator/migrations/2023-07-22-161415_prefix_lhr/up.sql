CREATE TABLE public.prefix_lhr(
    target_net cidr NOT NULL, -- network that was (attempted to be) reached & yielded this LHR. /64 in the beginning, but may be merged up
    router_ip inet NOT NULL,
    created_at timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    hit_count int NOT NULL,
    "data" jsonb NOT NULL DEFAULT '{}' ::jsonb,
    PRIMARY KEY (target_net, router_ip)
);

CREATE INDEX IF NOT EXISTS prefix_lhr_target_net_gist ON public.prefix_lhr USING gist(target_net inet_ops);

ALTER TABLE prefix_lhr
    ADD CONSTRAINT prefix_lhr_target_net_uq UNIQUE (target_net);

SELECT
    diesel_manage_updated_at('prefix_lhr');
