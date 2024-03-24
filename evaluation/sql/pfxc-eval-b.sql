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


SELECT masklen(net), (confidence > 100) as more_than_100, count(*) FROM prefix_tree pt
where merge_status in ('split_down', 'split_root')
group by masklen(net), (confidence > 100); -- B1' - overconfident splits by prefix len

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

