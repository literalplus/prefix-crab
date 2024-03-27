-- shared views
drop materialized view response_archive_responses_raw_short;
create materialized view response_archive_responses_raw
as
select
path, 
(("data"->'splits') is not null) as is_zmap,
(
	case when ("data"->'splits') is not null then -- zmap
		(("data"->'splits'->0->'responses')::jsonb || ("data"->'splits'->1->'responses')::jsonb )
	else '[]'::jsonb end
) as zmap_responses,
(
	case when ("data"->'results') is not null then -- yarrp
		(("data"->'results')::jsonb)
	else '[]'::jsonb end
) as yarrp_responses
from response_archive ra; commit;

-- subnet relationship detection
select ap.net, ap.asn, ap2.net, ap2.asn from 
	as_prefix ap 
	cross join as_prefix ap2 
where ap.net >>= ap2.net and ap.net != ap2.net;
--2001:890::/29	2001:890:c000::/34 -> both
--2001:628::/29	2001:628:2000::/48 -> both
--2001:628::/29	2001:628:453::/48 -> both
--2a03:3180::/36	2a03:3180:f::/48
--2a01:aea0::/32	2a01:aea0:df3::/48 -> AT-10 only
--2a01:aea0::/32	2a01:aea0:df4::/47 -> AT-10 only
--2a01:aea0::/32	2a01:aea0:dd4::/47 -> AT-10 only
--2a01:aea0::/32	2a01:aea0:dd3::/48 -> AT-10 only

create materialized view response_archive_per_path as
with all_combos as (
	select distinct path from response_archive_responses_raw
),
all_zmap as (
	select
		path,
		jsonb_agg(zmap_elems) as zmap_responses
	from
		response_archive_responses_raw ra,
		jsonb_array_elements(ra.zmap_responses) as zmap_elems
	where ra.is_zmap = true
	group by path
),
all_yarrp as (
	select
		path,
		jsonb_agg(yarrp_elems) as yarrp_responses
	from
		response_archive_responses_raw ra,
		jsonb_array_elements(ra.yarrp_responses) as yarrp_elems
	where ra.is_zmap = false
	group by path
)
select ac.path, az.zmap_responses, ay.yarrp_responses from
	all_combos ac
	left join all_zmap az on az.path = ac.path
	left join all_yarrp ay on ay.path = ac.path
;commit;

select
	path, is_zmap,
	jsonb_agg(zmap_elems) as zmap_responses,
	jsonb_agg(yarrp_elems) as yarrp_responses 
from
	response_archive_responses_raw_short ra,
	jsonb_array_elements(ra.zmap_responses) as zmap_elems,
	jsonb_array_elements(ra.yarrp_responses) as yarrp_elems
group by path, is_zmap
limit 10;

-- A1 histogram of entries per prefix length (what did we measure? how deep into prefixes?)
 
 -- analyze;
 
select * from response_archive ra
limit 100;

select count(*), masklen(path) from response_archive ra
group by masklen(path);--A1 Result

-- A2 how many zmap calls resulted in no response at all?
-- - = both nets have only one entry, and it has key NoResponse
-- - would be nice to evaluate this per AS / prefix, but not so easy; could group the info by /48 or so ...

select masklen(path), count(*) from response_archive ra
where ("data"->'splits') is not null and
jsonb_array_length("data"->'splits'->0->'responses') = 1 and
jsonb_array_length("data"->'splits'->1->'responses') = 1 and
"data"->'splits'->0->'responses'->0->'key' = '"NoResponse"' and
"data"->'splits'->1->'responses'->0->'key' = '"NoResponse"'
group by masklen(path)
;-- A2 result

select ('[{"a": "b"}]'::jsonb || '[{"c": 6}]'::jsonb);


--create materialized view response_archive_grouped_masklen as 
select masklen(path), 
--(("data"->'splits') is not null) as is_zmap,
count(*)
from response_archive ra
where (("data"->'splits') is not null)
group by masklen(path);


-- AT-11 total: 3037480
