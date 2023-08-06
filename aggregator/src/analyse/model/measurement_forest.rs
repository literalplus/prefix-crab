use std::{cmp::Ordering::{Equal, Greater, Less}, collections::HashSet, iter::{Chain, Map}, slice, vec, fmt::Display};

use crate::analyse::map64::{self, Net64Map};
use anyhow::{bail, Context, Result};
use ipnet::{IpNet, Ipv6Net};

use super::MeasurementTree;

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

#[derive(Eq, PartialEq, Debug)]
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
        let net = Self::validate_ipv6(&tree)?;
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

    fn validate_ipv6(tree: &MeasurementTree) -> Result<Ipv6Net> {
        match &tree.target_net {
            IpNet::V4(_) => bail!("i am the lorax. i speak for the trees. they do not want an IPv4 in their forest. thansk"),
            IpNet::V6(net) => Ok(*net),
        }
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
        let touched = if should_touch {ModificationType::Inserted} else {ModificationType::Untouched};
        match net.prefix_len().cmp(&map64::PREFIX_LEN) {
            Greater => bail!("trees with prefixes longer than /64 may not be planted here: {}. best regards, the lorax.", net),
            Equal => {
                let mut entry = self.trees64.entry_by_net_or(
                    &net, ModifiableTree::empty
                );
                entry.touched = touched;
                entry.consume_merge(tree, should_touch)?;
            }, 
            Less => self.merged_trees.push(ModifiableTree { tree, touched }),
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
