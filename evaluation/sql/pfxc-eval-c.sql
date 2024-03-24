-- C AS-level aggregates in prefix_tree (i.e. joined with as_prefix)

-- C1 SQL - prefix_tree only
create materialized view tree_with_root as
with roots as (
	select net, asn from as_prefix ap 
	where ap.deleted = false
)
select
	r.net as root_net,
	r.asn as root_asn,
	pt.net as subnet,
	pt.merge_status in ('unsplit_root', 'leaf', 'min_size_reached') as is_leaf,
	pt.confidence as confidence
from
	roots r 
	join prefix_tree pt
	on pt.net <<= r.net;
create index tree_with_root_root_idx on tree_with_root (root_net);

with leaf_stats as (
	select
		root_net,
		root_asn,
		count(*) as leaf_cnt
	from tree_with_root twr
	where 
		twr.is_leaf = true
	group by root_net, root_asn
),
leaf_255_stats as (
	select
		root_net, root_asn,
		count(*) as leaf_255_cnt,
		sum(2^(64-masklen(twr.subnet))) as confident_64s
	from tree_with_root twr
	where
		twr.is_leaf = true and
		(twr.confidence = 255 or masklen(twr.subnet) = 64)
	group by root_net, root_asn
),
deg64_stats as (
	select
		root_net, root_asn,
		count(*) as deg64_cnt
	from tree_with_root twr
	where
		masklen(twr.subnet) = 64
	group by root_net, root_asn
),
all_stats as (
	select
		root_net,
		root_asn,
		count(*) as nodes_cnt
	from tree_with_root twr
	group by root_net, root_asn
)
select
	als.root_net, als.root_asn,
	--
	masklen(als.root_net) as root_len,
	2^(64-masklen(als.root_net)) as size_64s,
	--
	nodes_cnt,
	--
	leaf_cnt,
	(leaf_cnt::float / nodes_cnt) as ratio_leaves,
	--
	leaf_255_cnt,
	(leaf_255_cnt::float / leaf_cnt) as ratio_255_of_leaves,
	confident_64s,
	(confident_64s::float / (2^(64-masklen(als.root_net)))) as ratio_confident_space,
	--
	deg64_cnt,
	(deg64_cnt::float / leaf_cnt) as ratio_64_of_leaves
from all_stats als
	left join leaf_stats ls
		on als.root_net = ls.root_net and als.root_asn = ls.root_asn
	left join leaf_255_stats l2s
		on als.root_net = l2s.root_net and als.root_asn = l2s.root_asn
	left join deg64_stats d64s
		on als.root_net = d64s.root_net and als.root_asn = d64s.root_asn
;--c1 final

-- C2 SQL - measurement_tree join

with roots as (
	select net, asn from as_prefix ap 
	where ap.deleted = false
)
select
	net,
	asn,
	sum(responsive_count) as sum_resp,
	sum(unresponsive_count) as sum_unresp
from roots r
	join measurement_tree mt
	on mt.target_net <<= r.net
group by net, asn;--c2 resp

with roots as (
	select net, asn from as_prefix ap 
	where ap.deleted = false
)
select
	net,
	asn,
	count(distinct jok) as overall_num_lhrs
from roots r
	join measurement_tree mt
	on mt.target_net <<= r.net,
	jsonb_object_keys(mt.last_hop_routers->'items') as jok
group by net, asn;--c2 overall lhr count

with roots as (
	select net, asn from as_prefix ap 
	where ap.deleted = false
),
root_metrs as (
	select distinct
		net as root_net,
		asn,
		mt.target_net as subnet,
		jsonb_object_keys(mt.last_hop_routers->'items') as lhrs
	from roots r
		join measurement_tree mt
		on mt.target_net <<= r.net
	order by asn, net, subnet, lhrs
),
lhr_sets_of_subnet as (
	select root_net, asn, subnet, jsonb_agg(lhrs) as lhr_set
	from root_metrs
	group by root_net, asn, subnet
),
distinct_lhrs_count as (
	select root_net, asn, count(distinct lhr_set)
	from lhr_sets_of_subnet
	group by root_net, asn
)
select * from distinct_lhrs_count;--c2 overall


-- C3 confident discoveries (leaves-up)
