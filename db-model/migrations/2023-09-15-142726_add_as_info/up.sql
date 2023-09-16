CREATE TABLE public.as_prefix
(
    net cidr primary key not null,
    deleted boolean not null default false,
    -- ASN are 32-bit unsigned integers, but postgres doesn't support unsigned, so
    -- we have to use bigint.
    -- We don't currently need the AS name / description, so we don't store it.
    asn bigint not null
);

CREATE INDEX prefix_bgp_status_asn ON public.as_prefix (asn);
