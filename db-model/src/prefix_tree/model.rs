use std::borrow::Borrow;
use std::net::Ipv6Addr;

use chrono::NaiveDateTime;
use diesel;
use diesel::pg::Pg;
use diesel::sql_types::*;
use diesel::{associations::HasTable, prelude::*};

use ipnet::{IpNet, Ipv6Net};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::analyse::Confidence;

#[derive(diesel_derive_enum::DbEnum, Debug, Copy, Clone, PartialEq, Eq)]
#[ExistingTypePath = "crate::sql_types::PrefixMergeStatus"]
pub enum MergeStatus {
    /// A leaf in the tree.
    /// Note that there may actually be subnets of type MergedUp below this if a merge has occurred.
    /// Analyses will be scheduled for this status.
    Leaf,
    /// A leaf with size /64.
    /// The main purpose of this is to stop probing these nets, as we can't find anything useful there.
    /// They have already been split to the maximum, so the only possible thing we could gain is merging
    /// them back up and that's not supported at the moment.
    MinSizeReached,
    /// A non-root node that was split down.
    SplitDown,
    /// A previously-leaf node that is a residue of a merge operation.
    /// We could also delete such nodes, but this at least leaves evidence for potential analysis.
    MergedUp,
    /// A root node that hasn't been split (yet).
    /// Analyses will be scheduled for this status.
    UnsplitRoot,
    /// A root node that behaves like [MergeStatus::SplitDown], but won't be merged further.
    SplitRoot,
    /// A node that will be ignored.
    /// There is no mechanism to automatically unblock such nodes.
    /// This is also the reason why there is no separate such status for root nodes.
    Blocked,
}

impl MergeStatus {
    pub fn is_eligible_for_split(&self) -> bool {
        matches!(self, MergeStatus::Leaf | MergeStatus::UnsplitRoot)
    }

    pub fn split(&self) -> MergeStatus {
        // NOTE - merge() logic is implemented in raw SQL in merge_redundant

        if matches!(self, MergeStatus::UnsplitRoot | MergeStatus::SplitRoot) {
            MergeStatus::SplitRoot
        } else if self == &MergeStatus::Blocked {
            MergeStatus::Blocked
        } else {
            MergeStatus::SplitDown
        }
    }

    pub fn new(child_prefix_len: u8) -> MergeStatus {
        if child_prefix_len >= 64 {
            MergeStatus::MinSizeReached
        } else {
            MergeStatus::Leaf
        }
    }
}

#[derive(
    Default,
    diesel_derive_enum::DbEnum,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    Copy,
    Clone,
    Hash,
    Ord,
    PartialOrd,
)]
#[ExistingTypePath = "crate::sql_types::PrefixPriorityClass"]
pub enum PriorityClass {
    // Important: Used in the database, do not change incompatibly!
    #[default]
    HighFresh,
    HighOverlapping,
    HighDisjoint,
    // same LHR set with multiple members (but different ratio)
    MediumSameMulti,
    // same LHR set with multiple members and same ratio (within reasonable margin)
    MediumSameRatio,
    // same single LHR
    MediumSameSingle,
    MediumMultiWeird,
    LowWeird,
    LowUnknown,
}

// The first 16 bytes of a SHA-256 hash of (sorted) IPv6 addresses,
// crammed into a UUID as if it were a general-purpose trash can and we were a raccoon
pub type LhrSetHash = uuid::Uuid;

#[derive(Queryable, Debug, Copy, Clone, PartialEq, Eq)]
#[diesel(table_name = crate::schema::prefix_tree)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(net))]
pub struct PrefixTree {
    #[diesel(deserialize_as = crate::persist::Ipv6NetLoader)]
    pub net: Ipv6Net,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,

    pub merge_status: MergeStatus,
    pub priority_class: PriorityClass,
    #[diesel(deserialize_as = crate::persist::ConfidenceLoader)]
    pub confidence: Confidence,

    /// A few things...
    /// 1.) 0u128 means unknown
    /// 2.) sha256 seems actually faster than md5 on modern servers
    /// 3.) u128 is just more ergonomic (we can store it as uuid in the DB)
    /// 4.) ... and even if cutting it off was problematic, we're not using it for crypto
    pub lhr_set_hash: LhrSetHash,

    /// Denormalised because lookup is nontrivial (tree traversal not just join),
    /// and we clear the prefix tree anyways on AS changes.
    /// Needed for AS-level rate limiting.
    pub asn: AsNumber,
}

impl HasTable for PrefixTree {
    type Table = crate::schema::prefix_tree::table;

    fn table() -> Self::Table {
        crate::schema::prefix_tree::table
    }
}

impl<'a> Identifiable for &'a PrefixTree {
    type Id = IpNet;

    fn id(self) -> Self::Id {
        IpNet::V6(self.net)
    }
}

use crate::schema::prefix_tree::dsl;
impl Selectable<Pg> for PrefixTree {
    type SelectExpression = (
        dsl::net,
        dsl::created_at,
        dsl::updated_at,
        dsl::merge_status,
        dsl::priority_class,
        dsl::confidence,
        dsl::lhr_set_hash,
        dsl::asn,
    );

    fn construct_selection() -> Self::SelectExpression {
        use dsl::*;
        (
            net,
            created_at,
            updated_at,
            merge_status,
            priority_class,
            confidence,
            lhr_set_hash,
            asn,
        )
    }
}

impl PrefixTree {
    pub fn hash_lhrs<I, T>(iter: I) -> LhrSetHash
    where
        I: Iterator<Item = T>,
        T: Borrow<Ipv6Addr> + Ord,
    {
        let mut lhrs = iter.into_iter().collect_vec();
        lhrs.sort_unstable();
        lhrs.dedup();
        let mut hasher = Sha256::new();
        for lhr in lhrs {
            hasher.update(lhr.borrow().octets());
        }
        LhrSetHash::from_slice(&hasher.finalize().as_slice()[0..16])
            .expect("[0..16] to have length 16")
    }
}

/// Currently chosen for simplicity since we can serialise this into the DB easily
/// with Diesel. Technically accurate would be u32, which doesn't fit into i32.
pub type AsNumber = i64;

#[derive(Queryable, Selectable, Debug, Copy, Clone)]
#[diesel(table_name = crate::schema::as_prefix)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(net))]
pub struct AsPrefix {
    #[diesel(deserialize_as = crate::persist::Ipv6NetLoader)]
    pub net: Ipv6Net,
    pub deleted: bool,
    pub asn: AsNumber,
}
