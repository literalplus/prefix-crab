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

CREATE TABLE public.as_filter_list
(
    asn bigint primary key not null,
    comment varchar(255) not null default ''
);

COMMENT ON TABLE public.as_filter_list IS 'semantic (allow/deny/ignore) depends on settings in seed-guard';

INSERT INTO public.as_filter_list (asn, comment) VALUES (64511, 'Local GNS3 testing, reserved ASN');
