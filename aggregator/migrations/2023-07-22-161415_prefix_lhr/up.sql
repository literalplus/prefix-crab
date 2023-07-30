CREATE TABLE public.measurement_tree(
    target_net cidr PRIMARY KEY NOT NULL, -- network that was (attempted to be) reached. /64 in the beginning, but may be merged up
    created_at timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    hit_count int NOT NULL, -- times that a response was observed (regardless which type; excl. follow-ups)
    miss_count int NOT NULL, -- times that a probe yielded no response
    last_hop_routers jsonb NOT NULL DEFAULT '{}' ::jsonb,
    weirdness jsonb NOT NULL DEFAULT '{}' ::jsonb
);

CREATE INDEX IF NOT EXISTS measurement_tree_target_net_gist ON public.measurement_tree USING gist(target_net inet_ops);

SELECT
    diesel_manage_updated_at('measurement_tree');
