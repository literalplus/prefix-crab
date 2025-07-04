use std::{
    collections::{HashMap, HashSet},
    net::Ipv6Addr,
    ops::IndexMut,
};

use anyhow::{bail, Result};
use chrono::{NaiveDateTime, Utc};
use diesel::{prelude::*, sql_types::Jsonb, AsExpression, FromSqlRow};
use ipnet::{IpNet, Ipv6Net};
use queue_models::probe_response::DestUnreachKind;
use serde::{Deserialize, Serialize};

use crate::{
    analyse::map64::Net64Map,
    prefix_tree::{LhrSetHash, PrefixTree},
};

pub type HitCount = i32;

#[derive(Queryable, Selectable, Identifiable, Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = crate::schema::measurement_tree)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(target_net))]
pub struct MeasurementTree {
    pub target_net: IpNet,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    /// Number of responses received from this prefix, including errors
    pub responsive_count: i32,
    pub unresponsive_count: i32,
    pub last_hop_routers: LhrData,
    pub weirdness: WeirdData,
}

impl MeasurementTree {
    pub fn empty(target_net: Ipv6Net) -> Self {
        Self {
            target_net: IpNet::V6(target_net),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            responsive_count: 0,
            unresponsive_count: 0,
            last_hop_routers: LhrData::default(),
            weirdness: WeirdData::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.responsive_count == 0 && self.unresponsive_count == 0
    }

    pub fn lhr_set_hash(&self) -> LhrSetHash {
        PrefixTree::hash_lhrs(self.last_hop_routers.items.keys())
    }

    /// Merges `other` into `self`, consuming the value.
    /// Fails if `other`'s network is not a subnet or the same as `self`'s.
    pub fn merge(&mut self, other: &Self) -> Result<()> {
        if !self.target_net.contains(&other.target_net) {
            bail!(
                "Cannot merge {:?} into {:?}, as the former is not a subnet-or-eq.",
                other,
                self
            );
        }
        self.updated_at = Utc::now().naive_utc();
        self.responsive_count = self.responsive_count.saturating_add(other.responsive_count);
        self.unresponsive_count = self
            .unresponsive_count
            .saturating_add(other.unresponsive_count);
        self.last_hop_routers
            .consume_merge(other.last_hop_routers.clone());
        self.weirdness.consume_merge(other.weirdness.clone());
        Ok(())
    }

    pub fn add_lhr_no_sum(&mut self, addr: Ipv6Addr, sources: HashSet<LhrSource>, hits: HitCount) {
        self.last_hop_routers.items.insert(
            addr,
            LhrItem {
                sources,
                hit_count: hits,
            },
        );
    }

    pub fn add_weird_no_sum(&mut self, description: WeirdType, hits: HitCount) {
        self.weirdness
            .items
            .insert(description, WeirdItem { hit_count: hits });
    }

    pub fn try_net_into_v6(&self) -> Result<Ipv6Net> {
        match &self.target_net {
            IpNet::V4(net) => bail!("i am the lorax. i speak for the trees. they do not want an IPv4 in their forest: {} thansk", net),
            IpNet::V6(net) => Ok(*net),
        }
    }
}

/// Last Hop Router in the context of a [MeasurementTree] node.
#[derive(FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Default, Clone)]
#[diesel(sql_type = Jsonb)]
pub struct LhrData {
    // IMPORTANT: Type must stay backwards-compatible with previously-written JSON,
    // i.e. add only optional fields or provide defaults!
    pub items: HashMap<Ipv6Addr, LhrItem>,
}

impl LhrData {
    fn consume_merge(&mut self, other: Self) {
        for (lhr_addr, item) in other.items.into_iter() {
            let entry = self.items.entry(lhr_addr).or_default();
            entry.consume_merge(item);
        }
    }

    pub fn sum_hits(&self) -> HitCount {
        let mut result = 0;
        for item in self.items.values() {
            result += item.hit_count;
        }
        result
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LhrSource {
    // IMPORTANT: Type must stay backwards-compatible with previously-written JSON,
    // i.e. add only optional fields or provide defaults!
    Trace,
    UnreachAdmin, // admin-prohibit, failed-egress
    UnreachPort,  // port unreach
    UnreachAddr,  // addr unreach
    UnreachRoute, // no route
}

impl TryFrom<&DestUnreachKind> for LhrSource {
    type Error = u8;

    fn try_from(value: &DestUnreachKind) -> Result<Self, Self::Error> {
        use DestUnreachKind as K;
        use LhrSource as S;

        Ok(match value {
            K::NoRoute => S::UnreachRoute,
            K::AdminProhibited => S::UnreachAdmin,
            K::AddressUnreachable => S::UnreachAddr,
            K::PortUnreachable => S::UnreachPort,
            K::Other(kind) => return Err(*kind),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct LhrItem {
    // IMPORTANT: Type must stay backwards-compatible with previously-written JSON,
    // i.e. add only optional fields or provide defaults!
    pub sources: HashSet<LhrSource>,
    pub hit_count: HitCount,
}

impl LhrItem {
    pub fn consume_merge(&mut self, other: Self) {
        self.sources.extend(other.sources);
        self.hit_count = self.hit_count.saturating_add(other.hit_count);
    }
}

crate::persist::configure_jsonb_serde!(LhrData);

#[derive(FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Default, Clone)]
#[diesel(sql_type = Jsonb)]
pub struct WeirdData {
    // IMPORTANT: Type must stay backwards-compatible with previously-written JSON,
    // i.e. add only optional fields or provide defaults!
    pub items: HashMap<WeirdType, WeirdItem>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum WeirdType {
    DestUnreachOther,
    DestUnreachRejectRoute,
    DestUnreachFailedEgress,
    DifferentEchoReplySource,
    EchoReplyInTrace,
    UnexpectedIcmpType,
    TtlExceededForEcho,
}

impl WeirdData {
    fn consume_merge(&mut self, other: Self) {
        for (description, other_item) in other.items.into_iter() {
            let entry = self.items.entry(description).or_default();
            entry.consume_merge(other_item);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct WeirdItem {
    // IMPORTANT: Type must stay backwards-compatible with previously-written JSON,
    // i.e. add only optional fields or provide defaults!
    pub hit_count: HitCount,
}

impl WeirdItem {
    pub fn consume_merge(&mut self, other: Self) {
        self.hit_count = self.hit_count.saturating_add(other.hit_count);
    }
}

crate::persist::configure_jsonb_serde!(WeirdData);

impl IndexMut<&Ipv6Net> for Net64Map<MeasurementTree> {
    fn index_mut(&mut self, idx: &Ipv6Net) -> &mut Self::Output {
        self.entry_by_net_or(idx, MeasurementTree::empty)
    }
}

impl IndexMut<&Ipv6Addr> for Net64Map<MeasurementTree> {
    fn index_mut(&mut self, idx: &Ipv6Addr) -> &mut Self::Output {
        self.entry_by_addr_or(idx, MeasurementTree::empty)
    }
}

#[cfg(test)]
mod tests {
    use assertor::{assert_that, EqualityAssertion, MapAssertion, ResultAssertion};
    use ipnet::Ipv6Net;
    use std::{net::Ipv6Addr, str::FromStr};

    use super::*;

    fn given_trees() -> (MeasurementTree, MeasurementTree) {
        let parent_tree = MeasurementTree::empty(Ipv6Net::from_str("2001:db8::/62").unwrap());
        let sub_tree = MeasurementTree::empty(Ipv6Net::from_str("2001:db8::/64").unwrap());
        (parent_tree, sub_tree)
    }

    fn given_some_addr() -> Ipv6Addr {
        Ipv6Addr::from_str("2001:db8::beef").unwrap()
    }

    fn given_another_addr() -> Ipv6Addr {
        Ipv6Addr::from_str("2001:db8::bee").unwrap()
    }

    #[test]
    fn merge_counts() {
        // given
        let (mut parent_tree, mut sub_tree) = given_trees();

        parent_tree.responsive_count = 7;
        sub_tree.responsive_count = 4;

        parent_tree.unresponsive_count = 3;
        sub_tree.unresponsive_count = 2;

        // when
        parent_tree.merge(&sub_tree).unwrap();

        // then
        assert_that!(parent_tree.responsive_count).is_equal_to(11);
        assert_that!(parent_tree.unresponsive_count).is_equal_to(5);
    }

    #[test]
    fn merge_lhrs() {
        // given
        let (mut parent_tree, mut sub_tree) = given_trees();

        parent_tree.last_hop_routers.items.insert(
            given_some_addr(),
            gen_lhr(6, &[LhrSource::Trace, LhrSource::UnreachAddr]),
        );
        sub_tree.last_hop_routers.items.insert(
            given_some_addr(),
            gen_lhr(2, &[LhrSource::Trace, LhrSource::UnreachPort]),
        );

        // when
        parent_tree.merge(&sub_tree).unwrap();

        // then
        let expected_item = gen_lhr(
            8,
            &[
                LhrSource::Trace,
                LhrSource::UnreachAddr,
                LhrSource::UnreachPort,
            ],
        );
        assert_that!(parent_tree.last_hop_routers.items)
            .contains_entry(given_some_addr(), expected_item);
        assert_that!(parent_tree.last_hop_routers.items).has_length(1);
    }

    fn gen_lhr(hit_count: HitCount, sources: &[LhrSource]) -> LhrItem {
        let mut item = LhrItem::default();
        item.hit_count = hit_count;
        item.sources.extend(sources);
        item
    }

    #[test]
    fn merge_weirds() {
        // given
        let (mut parent_tree, mut sub_tree) = given_trees();

        parent_tree
            .weirdness
            .items
            .insert(WeirdType::TtlExceededForEcho, WeirdItem { hit_count: 7 });
        sub_tree
            .weirdness
            .items
            .insert(WeirdType::TtlExceededForEcho, WeirdItem { hit_count: 2 });

        // when
        parent_tree.merge(&sub_tree).unwrap();

        // then
        let expected_item = WeirdItem { hit_count: 9 };
        assert_that!(parent_tree.weirdness.items)
            .contains_entry(WeirdType::TtlExceededForEcho, expected_item);
        assert_that!(parent_tree.weirdness.items).has_length(1);
    }

    #[test]
    fn no_merge_unrelated_addrs() {
        // given
        let (mut parent_tree, mut sub_tree) = given_trees();

        parent_tree
            .last_hop_routers
            .items
            .insert(given_some_addr(), gen_lhr(4, &[]));
        sub_tree
            .last_hop_routers
            .items
            .insert(given_another_addr(), gen_lhr(5, &[]));

        // when
        parent_tree.merge(&sub_tree).unwrap();
        // then
        assert_that!(parent_tree.last_hop_routers.items)
            .contains_entry(given_some_addr(), gen_lhr(4, &[]));
        assert_that!(parent_tree.last_hop_routers.items)
            .contains_entry(given_another_addr(), gen_lhr(5, &[]));
        assert_that!(parent_tree.last_hop_routers.items).has_length(2);
    }

    #[test]
    fn no_merge_unrelated_trees() {
        // given
        let (mut parent_tree, mut sub_tree) = given_trees();

        sub_tree.target_net = Ipv6Net::from_str("2001:db9::/62").unwrap().into();

        // when
        let result = parent_tree.merge(&sub_tree);
        // then
        assert_that!(result).is_err();
    }
}
