-- B1 splits with confidence > 100% (= measurements higher than 100 were needed to reach the conclusion) / = 100%
--    how many?
--    were the subnets then further split?

-- CREATE TYPE public.prefix_merge_status AS ENUM (
--	'leaf',
--	'split_down', X
--	'merged_up',
--	'unsplit_root',
--	'split_root', X
--	'min_size_reached',
--	'blocked');

SELECT (confidence > 100) as more_than_100, count(*) FROM prefix_tree pt
where merge_status in ('split_down', 'split_root')
group by (confidence > 100); -- B1

SELECT (confidence > 105) as more_than_105, count(*) FROM prefix_tree pt
where merge_status in ('split_down', 'split_root')
group by (confidence > 105); -- B1

SELECT masklen(net), (confidence > 100) as more_than_100, count(*) FROM prefix_tree pt
where merge_status in ('split_down', 'split_root')
group by masklen(net), (confidence > 100); -- B1' - overconfident splits by prefix len

with all_conf as (
	select confidence, count(*) as all_nodes from prefix_tree pt 
	group by confidence
	order by confidence desc -- B1'' - confidence distribution
),
split_conf as (
	select confidence, count(*) as split_nodes from prefix_tree pt 
	where merge_status in ('split_down', 'split_root')
	group by confidence
	order by confidence desc -- B1'' - confidence distribution for splits
)
select
	(width_bucket(confidence, 0, 256, 17)-1)*(255.0/17) as bucket,
	SUM(all_nodes) as all_nodes,
	SUM(split_nodes) as split_nodes
from all_conf natural full outer join split_conf
group by bucket
order by bucket desc;


-- B2 node prefix length distribution (SQL prefix_tree)
select masklen(net), count(*) from prefix_tree pt
group by masklen(net);

-- B3 leaf prefix length distribution (SQL prefix_tree)
select masklen(net), count(*) from prefix_tree pt
where merge_status in ('leaf', 'min_size_reached', 'unsplit_root')
group by masklen(net);

-- B3' leaf prefix length distribution (SQL prefix_tree) - confidence 100 or more
select masklen(net), count(*) from prefix_tree pt
where merge_status in ('leaf', 'min_size_reached', 'unsplit_root')
and confidence >= 100
group by masklen(net);

-- extra: split_analyses per prefix length
select case when masklen(tree_net) > 40 then 'smaller' when masklen(tree_net) > 36 then 'range' else 'larger' end, count(*) from split_analysis sa 
group by case when masklen(tree_net) > 40 then 'smaller' when masklen(tree_net) > 36 then 'range' else 'larger' end ;

-- U-3
-- smaller	2413
-- range	6699
-- larger	1007
