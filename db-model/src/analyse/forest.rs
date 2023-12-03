use std::{cmp::Ordering::{Equal, Greater, Less}, collections::HashSet, iter::{Chain, Map}, slice, vec, fmt::Display};

use crate::analyse::map64::{self, Net64Map};
use anyhow::{bail, Context, Result};
use ipnet::{IpNet, Ipv6Net};

use super::tree::MeasurementTree;

#[derive(Default, Debug)]
pub struct MeasurementForest {
    trees64: Net64Map<ModifiableTree>,
    merged_trees: Vec<ModifiableTree>,
    pub obsolete_nets: HashSet<Ipv6Net>,
}

impl Display for MeasurementForest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeasurementForest")
            .field("watcher", &"The Lorax")
            .field("trees64", &format!("{} entrees", self.trees64.len()))
            .field("merged_trees", &format!("{} entrees", self.merged_trees.len()))
            .field("obsolete_nets", &self.obsolete_nets)
            .finish()
    }
}

#[derive(Debug)]
pub struct ModifiableTree {
    pub tree: MeasurementTree,
    pub touched: ModificationType,
}

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum ModificationType {
    Untouched,
    Inserted,
    Updated,
}

impl ModifiableTree {
fn empty(net: Ipv6Net) -> Self {
    ModifiableTree { tree: MeasurementTree::empty(net), touched: ModificationType::Untouched }
}

fn consume_merge(&mut self, other: MeasurementTree, should_touch: bool) -> Result<()> {
    self.tree
        .consume_merge(other)
        .context("merge failed :(")?;
    if should_touch && self.touched == ModificationType::Untouched {
        self.touched = ModificationType::Updated;
    }
    Ok(())
}

/// Panics if the stored network is IPv4
fn expect_ipv6_net(&self) -> Ipv6Net {
    match &self.tree.target_net {
        IpNet::V4(net) => panic!("expected inserting code (the lorax) to keep IPv4s out of our forest, but found: {}", net),
        IpNet::V6(net) => *net,
    }
}
}

impl MeasurementForest {
pub fn with_untouched(trees: Vec<MeasurementTree>) -> Result<Self> {
    let mut me = Self::default();
    for tree in trees {
        me.insert_with_touch(tree, false)?;
    }
    Ok(me)
}

/// Returns whether this network or a supernetwork was already present in the tree.
/// Fails if an IPv4 network was passed.
pub fn insert(&mut self, tree: MeasurementTree) -> Result<()> {
    self.insert_with_touch(tree, true)
}

fn insert_with_touch(&mut self, tree: MeasurementTree, should_touch: bool) -> Result<()> {
    let net = tree.try_net_into_v6()?;
    match self.merge_into_existing_or_return(tree, should_touch)? {
        Some(my_tree) => self.insert_no_merge(net, my_tree, should_touch)?,
        None => {
            if !should_touch {
                self.obsolete_nets.insert(net);
            }
        },
    };
    Ok(())
}

fn merge_into_existing_or_return(&mut self, tree: MeasurementTree, should_touch: bool) -> Result<Option<MeasurementTree>> {
    for merged_candidate in self.merged_trees.iter_mut() {
        if merged_candidate.tree.target_net.contains(&tree.target_net) {
            merged_candidate.consume_merge(tree, should_touch)?;
            return Ok(None);
        }
    }
    Ok(Some(tree))
}

fn insert_no_merge(&mut self, net: Ipv6Net, tree: MeasurementTree, should_touch: bool) -> Result<()> {
    let touched_no_conflict = if should_touch {
        ModificationType::Inserted
    } else {
        ModificationType::Untouched
    };
    match net.prefix_len().cmp(&map64::PREFIX_LEN) {
        Greater => bail!("trees with prefixes longer than /64 may not be planted here: {}. best regards, the lorax.", net),
        Equal => {
            let was_there_before = self.trees64.contains_net(&net);
            let entry = self.trees64.entry_by_net_or(
                &net, ModifiableTree::empty
            );
            entry.touched = if was_there_before {
                if should_touch {
                    ModificationType::Updated
                } else {
                    entry.touched
                }
            } else {
                touched_no_conflict
            };
            entry.consume_merge(tree, should_touch)?;
        }, 
        Less => self.merged_trees.push(ModifiableTree { tree, touched: touched_no_conflict }),
    };
    Ok(())
}
}

type IntoIterTouched = Chain<vec::IntoIter<ModifiableTree>, map64::IntoIterValues<ModifiableTree>>;
type IterNets<'a> = Map<Chain<slice::Iter<'a, ModifiableTree>, map64::IterValues<'a, ModifiableTree>>, fn(&ModifiableTree) -> Ipv6Net>;

impl<'a> MeasurementForest {
pub fn into_iter_touched(self) -> IntoIterTouched {
    self.merged_trees.into_iter()
        .chain(self.trees64.into_iter_values())
}

pub fn to_iter_all_nets(&'a self) -> IterNets<'a> {
    self.merged_trees.iter()
        .chain(self.trees64.iter_values())
        .map(move |it| it.expect_ipv6_net())
}
}

#[cfg(test)]
mod tests {
use assertor::*;
use itertools::Itertools;

use super::*;
use crate::test_utils::*;

#[test]
fn insert_touch_fresh() {
    // given
    let tree = gen_tree_with_lhr_beef(TREE_LEFT_NET, 4);
    let mut forest = MeasurementForest::default();
    // when
    forest.insert(tree).unwrap();
    // then
    let tree_again = forest.into_iter_touched().next().unwrap();
    assert_that!(tree_again.touched).is_equal_to(ModificationType::Inserted);
}

#[test]
fn insert_touch_untouched() {
    // given
    let tree = gen_tree_with_lhr_beef(TREE_LEFT_NET, 4);
    let forest = MeasurementForest::with_untouched(vec![tree]).unwrap();
    // when nothing happens :)
    // then
    let tree_again = forest.into_iter_touched().next().unwrap();
    assert_that!(tree_again.touched).is_equal_to(ModificationType::Untouched);
}

#[test]
fn insert_touch_updated() {
    // given
    let tree = gen_tree_with_lhr_beef(TREE_LEFT_NET, 4);
    let mut forest = MeasurementForest::with_untouched(vec![tree]).unwrap();
    // when
    forest.insert(gen_tree_with_lhr_101(TREE_LEFT_NET, 7)).unwrap();
    // then
    let tree_again = forest.into_iter_touched().next().unwrap();
    assert_that!(tree_again.touched).is_equal_to(ModificationType::Updated);
    assert_that!(tree_again.tree.last_hop_routers.items).has_length(2);
}

#[test]
fn insert_touch_updated_larger_net() {
    // given
    let tree = gen_tree_with_lhr_beef(TREE_BASE_NET, 4);
    let mut forest = MeasurementForest::with_untouched(vec![tree]).unwrap();
    // when
    forest.insert(gen_tree_with_lhr_101(TREE_LEFT_NET, 7)).unwrap();
    // then
    let tree_again = forest.into_iter_touched().next().unwrap();
    assert_that!(tree_again.touched).is_equal_to(ModificationType::Updated);
    assert_that!(tree_again.tree.last_hop_routers.items).has_length(2);
    assert_that!(tree_again.tree.target_net).is_equal_to(IpNet::V6(net(TREE_BASE_NET)));
}

#[test]
fn insert_dont_care_unrelated_merge_net() {
    // given
    let tree = gen_tree_with_lhr_beef(TREE_BASE_NET, 4);
    let mut forest = MeasurementForest::with_untouched(vec![tree]).unwrap();
    // when
    forest.insert(gen_tree_with_lhr_101(TREE_UNRELATED_NET, 7)).unwrap();
    // then
    assert_that!(forest.into_iter_touched().collect_vec()).has_length(2);
}

#[test]
fn insert_dont_care_unrelated_64_net() {
    // given
    let tree = gen_tree_with_lhr_beef(TREE_RIGHT_NET, 4);
    let mut forest = MeasurementForest::with_untouched(vec![tree]).unwrap();
    // when
    forest.insert(gen_tree_with_lhr_101(TREE_LEFT_NET, 7)).unwrap();
    // then
    assert_that!(forest.into_iter_touched().collect_vec()).has_length(2);
}
}
