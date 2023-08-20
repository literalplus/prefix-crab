use std::{
    array::from_fn,
    collections::{HashMap, HashSet},
    net::Ipv6Addr,
    ops::Deref,
};

use anyhow::{Context, Result};
use ipnet::{IpNet, Ipv6Net};
use log::warn;
use prefix_crab::prefix_split::{self, SplitSubnet};

use crate::analyse::{LhrAddr, LhrItem};

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

pub struct Subnets([Subnet; 2]);

impl Subnets {
    pub fn new(base_net: Ipv6Net, relevant_measurements: Vec<MeasurementTree>) -> Result<Self> {
        let split = prefix_split::split(base_net).context("trying to split for split analysis")?;
        let mut result: [Subnet; 2] = split.into_subnets().map(From::from);
        let split_nets: [IpNet; 2] = from_fn(|i| IpNet::V6(*&result[i].subnet.network));
        for tree in relevant_measurements {
            let mut unused_tree = Some(tree);
            for (i, subnet) in result.iter_mut().enumerate() {
                let net_borrow = &unused_tree.as_ref().expect("tree for net").target_net;
                if net_borrow <= &split_nets[i] {
                    subnet.synthetic_tree.consume_merge(unused_tree.take().expect("tree for merge"));
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
        Ok(Self(result))
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Subnet> {
        self.0.iter()
    }

    fn left(&self) -> &Subnet {
        &self.0[0]
    }

    fn right(&self) -> &Subnet {
        &self.0[1]
    }

    pub fn lhr_diff(&self) -> LhrSetDifference {
        use LhrSetDifference::*;

        let mut data_map: HashMap<&LhrAddr, LhrItem> = HashMap::new();
        let mut addr_sets: [HashSet<&LhrAddr>; 2] = Default::default();
        for (i, subnet) in self.iter().enumerate() {
            for (addr, data) in subnet.iter_lhrs() {
                let entry = data_map.entry(addr).or_default();
                entry.consume_merge(data.clone());
                addr_sets[i].insert(addr);
            }
        }

        let [left_set, right_set] = addr_sets;
        if left_set == right_set {
            return match left_set.len() {
                0 => BothNone,
                1 => BothSameSingle {
                    lhr: data_map.into_values().next().expect("to find only LHR"),
                },
                _ => BothSameMultiple {
                    lhrs: data_map.into_values().collect(),
                },
            };
        }
        // If we implement `difference` & `intersection` ourselves in one step, we could skip the
        // clone() on the key; doesn't seem worth it atm but could improve performance in the future.
        let distinct: Vec<LhrItem> = left_set
            .difference(&right_set)
            .map(|addr| data_map[addr].clone())
            .collect();
        let shared: Vec<LhrItem> = left_set
            .intersection(&right_set)
            .map(|addr| data_map[addr].clone())
            .collect();

        return if shared.is_empty() {
            if distinct.is_empty() {
                BothNone
            } else {
                Disjoint { lhrs: distinct }
            }
        } else {
            if distinct.is_empty() {
                if shared.len() == 1 {
                    BothSameSingle {
                        lhr: shared
                            .into_iter()
                            .next()
                            .expect("vec with length one to have item"),
                    }
                } else {
                    BothSameMultiple { lhrs: shared }
                }
            } else {
                Overlapping { shared, distinct }
            }
        };
    }
}

impl Deref for Subnets {
    type Target = [Subnet; 2];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub enum LhrSetDifference {
    BothNone,
    BothSameSingle {
        lhr: LhrItem,
    },
    BothSameMultiple {
        lhrs: Vec<LhrItem>,
    },
    Overlapping {
        shared: Vec<LhrItem>,
        distinct: Vec<LhrItem>,
    },
    Disjoint {
        lhrs: Vec<LhrItem>,
    },
}
