use std::{
    array::from_fn,
    collections::{HashMap, HashSet},
    hash::Hash,
    ops::Deref,
};

use anyhow::{Context, Result};
use db_model::analyse::LhrSource;
use ipnet::{IpNet, Ipv6Net};
use log::warn;
use prefix_crab::prefix_split::{self, SplitSubnet};

use crate::analyse::{HitCount, LhrAddr, LhrItem, WeirdItem, WeirdType};

use super::MeasurementTree;

#[derive(Debug)]
pub struct Subnet {
    pub subnet: SplitSubnet,
    /// All measurement trees found in this subnet, merged into one.
    synthetic_tree: MeasurementTree,
}

impl Subnet {
    fn iter_lhrs(&self) -> std::collections::hash_map::Iter<'_, LhrAddr, LhrItem> {
        self.synthetic_tree.last_hop_routers.items.iter()
    }

    fn iter_weirds(&self) -> std::collections::hash_map::Iter<'_, WeirdType, WeirdItem> {
        self.synthetic_tree.weirdness.items.iter()
    }
}

impl From<SplitSubnet> for Subnet {
    fn from(subnet: SplitSubnet) -> Self {
        let synthetic_tree = MeasurementTree::empty(subnet.network);
        Self {
            subnet,
            synthetic_tree,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct LhrDiff {
    pub sources: HashSet<LhrSource>,
    // Important: Multiple places in the code assume the exact length 2 !
    pub hit_counts: [HitCount; 2],
}

impl LhrDiff {
    fn consume(&mut self, subnet_id: usize, item: LhrItem) {
        self.hit_counts[subnet_id] = item.hit_count;
        self.sources.extend(item.sources);
    }

    pub fn total_hit_count(&self) -> HitCount {
        self.hit_counts[0].saturating_add(self.hit_counts[1])
    }
}

#[derive(Debug)]
pub struct Subnets {
    splits: [Subnet; 2],
}

impl Subnets {
    pub fn new(base_net: Ipv6Net, relevant_measurements: &[MeasurementTree]) -> Result<Self> {
        let split = prefix_split::split(base_net).context("trying to split for split analysis")?;
        let mut splits: [Subnet; 2] = split.into_subnets().map(From::from);
        let split_nets: [IpNet; 2] = from_fn(|i| IpNet::V6(splits[i].subnet.network));
        for tree in relevant_measurements {
            let mut unused_tree = Some(tree);
            for (i, candidate_split) in splits.iter_mut().enumerate() {
                let tree_net = &unused_tree.as_ref().expect("tree for net").target_net;
                if split_nets[i].contains(tree_net) {
                    candidate_split
                        .synthetic_tree
                        .merge(unused_tree.take().expect("tree for merge"))?;
                    break;
                }
            }
            if let Some(unused) = unused_tree {
                warn!(
                    "Received a tree that didn't fit into either subnet: {:?} - {:?}",
                    split_nets, unused,
                );
            }
        }
        Ok(Self { splits })
    }

    pub fn lhr_diff(&self) -> Diff<LhrDiff> {
        let mut lookup: HashMap<&LhrAddr, LhrDiff> = HashMap::new();
        let mut addr_sets: [HashSet<&LhrAddr>; 2] = Default::default();
        for (i, subnet) in self.iter().enumerate() {
            for (addr, data) in subnet.iter_lhrs() {
                let entry = lookup.entry(addr).or_default();
                entry.consume(i, data.clone());
                addr_sets[i].insert(addr);
            }
        }

        lookup_diff(addr_sets, lookup)
    }

    pub fn weird_diff(&self) -> Diff<WeirdItem> {
        let mut lookup: HashMap<&WeirdType, WeirdItem> = HashMap::new();
        let mut type_sets: [HashSet<&WeirdType>; 2] = Default::default();

        for (i, subnet) in self.iter().enumerate() {
            for (addr, data) in subnet.iter_weirds() {
                let entry = lookup.entry(addr).or_default();
                entry.consume_merge(data.clone());
                type_sets[i].insert(addr);
            }
        }

        lookup_diff(type_sets, lookup)
    }

    pub fn sum_subtrees(&self, count_fn: fn(&MeasurementTree) -> HitCount) -> HitCount {
        let mut result = 0;
        for subnet in self.iter() {
            result += count_fn(&subnet.synthetic_tree);
        }
        result
    }
}

impl Deref for Subnets {
    type Target = [Subnet; 2];

    fn deref(&self) -> &Self::Target {
        &self.splits
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Diff<I> {
    BothNone,
    BothSameSingle { shared: I },
    BothSameMultiple { shared: Vec<I> },
    OverlappingOrDisjoint { shared: Vec<I>, distinct: Vec<I> },
}

fn lookup_diff<K, I>(sets: [HashSet<K>; 2], lookup: HashMap<K, I>) -> Diff<I>
where
    K: Hash + Eq + PartialEq + std::fmt::Debug,
    I: Clone,
{
    let [left_set, right_set] = sets;
    // If we implement `difference` & `intersection` ourselves in one step, we could skip the
    // clone() on the key; doesn't seem worth it atm but could improve performance in the future.
    let distinct: Vec<I> = left_set
        .symmetric_difference(&right_set)
        .map(|k| {
            lookup
                .get(k)
                .unwrap_or_else(|| panic!("lookup should have {:?} (a distinct key)", k))
                .clone()
        })
        .collect();
    let shared: Vec<I> = left_set
        .intersection(&right_set)
        .map(|k| {
            lookup
                .get(k)
                .unwrap_or_else(|| panic!("lookup should have {:?} (a shared key)", k))
                .clone()
        })
        .collect();
    Diff::from(shared, distinct)
}

impl<I> Diff<I> {
    fn from(shared: Vec<I>, distinct: Vec<I>) -> Self {
        use Diff::*;

        if shared.is_empty() {
            if distinct.is_empty() {
                BothNone
            } else {
                OverlappingOrDisjoint {
                    shared: vec![],
                    distinct,
                }
            }
        } else if distinct.is_empty() {
            if shared.len() == 1 {
                BothSameSingle {
                    shared: shared
                        .into_iter()
                        .next()
                        .expect("vec with length one to have item"),
                }
            } else {
                BothSameMultiple { shared }
            }
        } else {
            OverlappingOrDisjoint { shared, distinct }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use assertor::{assert_that, EqualityAssertion, MapAssertion, IteratorAssertion};

    use super::*;
    use crate::analyse::LhrSource;
    use db_model::test_utils::*;

    #[test]
    fn new_empty() {
        // given
        let base_net = net(TREE_BASE_NET);
        let relevant_measurements = vec![];

        // when
        let subnets = Subnets::new(base_net, &relevant_measurements).unwrap();

        // then
        assert_that!(subnets.iter()).has_length(2);
    }

    #[test]
    fn new_check_merge() {
        // given
        let base_net = net(TREE_BASE_NET);
        let relevant_measurements = gen_measurements_complex();

        // when
        let subnets = Subnets::new(base_net, &relevant_measurements).unwrap();

        // then
        let [left, right] = subnets.deref();
        then_contains_lhr_101(&left, 14);
        then_contains_lhr_beef(&right, 24);
        then_contains_lhr_101(&right, 9);
        then_lhr_count(&left, 1);
        then_lhr_count(&right, 2);
    }

    fn then_contains_lhr_101(sub: &Subnet, hit_count: HitCount) {
        assert_that!(sub.synthetic_tree.last_hop_routers.items).contains_entry(
            addr(TREE_LHR_101),
            LhrItem {
                hit_count,
                sources: vec![LhrSource::UnreachAddr].into_iter().collect(),
            },
        );
    }

    fn then_contains_lhr_beef(sub: &Subnet, hit_count: HitCount) {
        assert_that!(sub.synthetic_tree.last_hop_routers.items).contains_entry(
            addr(TREE_LHR_BEEF),
            LhrItem {
                hit_count,
                sources: vec![LhrSource::Trace].into_iter().collect(),
            },
        );
    }

    fn then_lhr_count(sub: &Subnet, expected: usize) {
        assert_that!(sub.synthetic_tree.last_hop_routers.items).has_length(expected);
    }

    #[test]
    fn new_check_disjoint() {
        // given
        let base_net = net(TREE_BASE_NET);
        let relevant_measurements = vec![
            gen_tree_with_lhr_101(TREE_LEFT_NET, 2),
            gen_tree_with_lhr_beef(TREE_RIGHT_NET, 3),
            gen_tree_with_lhr_beef(TREE_RIGHT_NET_ALT, 3),
        ];

        // when
        let subnets = Subnets::new(base_net, &relevant_measurements).unwrap();

        // then
        let [left, right] = subnets.deref();
        then_contains_lhr_101(&left, 2);
        then_contains_lhr_beef(&right, 6);
        then_lhr_count(&left, 1);
        then_lhr_count(&right, 1);
    }

    #[test]
    fn diff_from_both_none() {
        // given
        let shared: Vec<i32> = vec![];
        let distinct: Vec<i32> = vec![];

        // when
        let diff = Diff::from(shared, distinct);

        // then
        assert_that!(diff).is_equal_to(Diff::BothNone);
    }

    #[test]
    fn diff_from_disjoint() {
        // given
        let shared: Vec<i32> = vec![];
        let distinct: Vec<i32> = vec![12];

        // when
        let diff = Diff::from(shared.clone(), distinct.clone());

        // then
        assert_that!(diff).is_equal_to(Diff::OverlappingOrDisjoint { shared, distinct });
    }

    #[test]
    fn diff_from_distinct_with_shared() {
        // given
        let shared: Vec<i32> = vec![4];
        let distinct: Vec<i32> = vec![12];

        // when
        let diff = Diff::from(shared.clone(), distinct.clone());

        // then
        assert_that!(diff).is_equal_to(Diff::OverlappingOrDisjoint { shared, distinct });
    }

    #[test]
    fn diff_from_same_single() {
        // given
        let shared: Vec<i32> = vec![4];
        let distinct: Vec<i32> = vec![];

        // when
        let diff = Diff::from(shared.clone(), distinct.clone());

        // then
        assert_that!(diff).is_equal_to(Diff::BothSameSingle { shared: 4 });
    }

    #[test]
    fn diff_from_same_multi() {
        // given
        let shared: Vec<i32> = vec![4, 8];
        let distinct: Vec<i32> = vec![];

        // when
        let diff = Diff::from(shared.clone(), distinct.clone());

        // then
        assert_that!(diff).is_equal_to(Diff::BothSameMultiple { shared });
    }

    #[test]
    fn diff_lookup_both_set() {
        // given
        let left_set: HashSet<i32> = vec![4, 8].into_iter().collect();
        let right_set: HashSet<i32> = vec![4, 12].into_iter().collect();
        let mut lookup = HashMap::new();
        lookup.insert(4, "shared");
        lookup.insert(8, "left only");
        lookup.insert(12, "right only");

        // when
        let diff = lookup_diff([left_set, right_set], lookup);

        // then
        assert_that!(diff).is_equal_to(Diff::OverlappingOrDisjoint {
            shared: vec!["shared"],
            distinct: vec!["left only", "right only"],
        });
    }
}
