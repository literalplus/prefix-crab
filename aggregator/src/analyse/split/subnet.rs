use std::{
    array::from_fn,
    collections::{HashMap, HashSet},
    hash::Hash,
    net::Ipv6Addr,
    ops::Deref,
};

use anyhow::{Context, Result};
use ipnet::{IpNet, Ipv6Net};
use log::warn;
use prefix_crab::prefix_split::{self, SplitSubnet};

use crate::analyse::{HitCount, LhrAddr, LhrItem, WeirdItem, WeirdType};

use super::MeasurementTree;

pub struct Subnet {
    pub subnet: SplitSubnet,
    /// All measurement trees found in this subnet, merged into one.
    pub synthetic_tree: MeasurementTree,
}

impl Subnet {
    pub fn iter_lhrs(&self) -> std::collections::hash_map::Iter<'_, LhrAddr, LhrItem> {
        self.synthetic_tree.last_hop_routers.items.iter()
    }

    pub fn iter_weirds(&self) -> std::collections::hash_map::Iter<'_, WeirdType, WeirdItem> {
        self.synthetic_tree.weirdness.items.iter()
    }
}

impl From<SplitSubnet> for Subnet {
    fn from(subnet: SplitSubnet) -> Self {
        let synthetic_tree = MeasurementTree::empty(subnet.network.clone());
        Self {
            subnet,
            synthetic_tree,
        }
    }
}

pub struct Subnets {
    net: Ipv6Net,
    splits: [Subnet; 2],
}

impl Subnets {
    pub fn new(base_net: Ipv6Net, relevant_measurements: Vec<MeasurementTree>) -> Result<Self> {
        let split = prefix_split::split(base_net).context("trying to split for split analysis")?;
        let mut splits: [Subnet; 2] = split.into_subnets().map(From::from);
        let split_nets: [IpNet; 2] = from_fn(|i| IpNet::V6(*&splits[i].subnet.network));
        for tree in relevant_measurements {
            let mut unused_tree = Some(tree);
            for (i, subnet) in splits.iter_mut().enumerate() {
                let net_borrow = &unused_tree.as_ref().expect("tree for net").target_net;
                if net_borrow <= &split_nets[i] {
                    subnet
                        .synthetic_tree
                        .consume_merge(unused_tree.take().expect("tree for merge"))?;
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
        Ok(Self {
            net: base_net,
            splits,
        })
    }

    fn left(&self) -> &Subnet {
        &self.splits[0]
    }

    fn right(&self) -> &Subnet {
        &self.splits[1]
    }

    pub fn lhr_diff(&self) -> Diff<LhrItem> {
        let mut lookup: HashMap<&LhrAddr, LhrItem> = HashMap::new();
        let mut addr_sets: [HashSet<&LhrAddr>; 2] = Default::default();
        for (i, subnet) in self.iter().enumerate() {
            for (addr, data) in subnet.iter_lhrs() {
                let entry = lookup.entry(addr).or_default();
                entry.consume_merge(data.clone());
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

    // pub fn as_synthetic_tree(&self) -> MeasurementTree {
    //     let mut tree = MeasurementTree::empty(self.net);
    //     for subnet in self.iter() {
    //         tree.consume_merge(subnet.synthetic_tree.clone());
    //     }
    //     tree
    // }

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

pub enum Diff<I> {
    BothNone,
    BothSameSingle { shared: I },
    BothSameMultiple { shared: Vec<I> },
    OverlappingOrDisjoint { shared: Vec<I>, distinct: Vec<I> },
}

fn lookup_diff<K, I>(sets: [HashSet<K>; 2], lookup: HashMap<K, I>) -> Diff<I>
where
    K: Hash + Eq + PartialEq,
    I: Clone,
{
    let [left_set, right_set] = sets;
    // If we implement `difference` & `intersection` ourselves in one step, we could skip the
    // clone() on the key; doesn't seem worth it atm but could improve performance in the future.
    let distinct: Vec<I> = left_set
        .difference(&right_set)
        .map(|k| lookup[k].clone())
        .collect();
    let shared: Vec<I> = left_set
        .intersection(&right_set)
        .map(|k| lookup[k].clone())
        .collect();
    Diff::from(shared, distinct)
}

impl<I> Diff<I> {
    fn from(shared: Vec<I>, distinct: Vec<I>) -> Self {
        use Diff::*;

        return if shared.is_empty() {
            if distinct.is_empty() {
                BothNone
            } else {
                OverlappingOrDisjoint {
                    shared: vec![],
                    distinct,
                }
            }
        } else {
            if distinct.is_empty() {
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
        };
    }
}
