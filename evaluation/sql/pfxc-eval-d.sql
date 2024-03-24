-- D1 overall response rate % (not super meaningful without splitting into zmap and yarrp)
SELECT sum(responsive_count), sum(unresponsive_count) FROM measurement_tree mt;

-- D2 overall number of /64s hit
select masklen(target_net), count(*)
from measurement_tree mt
group by masklen(target_net );

