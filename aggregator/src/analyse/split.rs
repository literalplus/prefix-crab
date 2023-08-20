use std::{array::from_fn, cmp::Reverse, net::Ipv6Addr};

use anyhow::{Context, Result};
use diesel::{prelude::*, PgConnection, QueryDsl, SelectableHelper};
use ipnet::{IpNet, Ipv6Net};
use itertools::Itertools;
use log::warn;
use prefix_crab::prefix_split::{self, PrefixSplit, SplitSubnet};

use crate::{
    persist::dsl::CidrMethods,
    prefix_tree::ContextOps,
    schema::measurement_tree::{dsl::measurement_tree, target_net},
};

use self::subnet::{Subnets, Subnet};

use super::{context, LhrItem, MeasurementTree};

mod subnet;

pub fn process(conn: &mut PgConnection, request: context::Context) -> Result<()> {
    let relevant_measurements = measurement_tree
        .filter(target_net.subnet_or_eq(request.node().path))
        .select(MeasurementTree::as_select())
        .load(conn)
        .with_context(|| {
            format!(
                "Unable to load existing measurements for {}",
                request.log_id()
            )
        })?;
    let base_net = request.node().try_net_into_v6()?;
    let subnets = Subnets::new(base_net, relevant_measurements)?;
    Ok(())
}

enum SplitAction {
    /// Two different subnets have been detected & a split is suggested
    YesSplit,
    /// The two subnets look similar & a split is not suggested.
    NoKeep { consider_merge: bool },
    /// An action could not be determined
    CannotDetermine { maybe_with_more_data: bool },
}

type SplitConfidence = i32;

struct SplitRecommendation {
    pub action: SplitAction,
    pub confidence: SplitConfidence,
}

impl SplitRecommendation {
    fn new(subnets: Subnets) {
        use self::subnet::LhrSetDifference::*;

        match subnets.lhr_diff() {
            BothNone => {

            },
            BothSameSingle { lhr } => todo!(),
            BothSameMultiple { lhrs } => todo!(),
            Overlapping { shared, distinct } => todo!(),
            Disjoint { lhrs } => todo!(),
        }
    }
}


// impl TryFrom<Subnet> for (SubnetInterpretation, i32) {
//     type Error = anyhow::Error;

//     fn try_from(value: Subnet) -> Result<Self> {
//         use SubnetInterpretation::*;

//         let mut tree = MeasurementTree::empty(value.subnet.network);
//         for found_tree in value.trees {
//             tree.consume_merge(found_tree)?;
//         }
//         let lhrs_most_to_least_hits = tree
//             .last_hop_routers
//             .items
//             .iter()
//             .sorted_unstable_by_key(|(addr, item)| Reverse(item.hit_count));
//         let mut most_hits = 0u32;
//         let mut other_hits = 0u32;
//         for (_, lhr) in lhrs_most_to_least_hits {
//             if most_hits == 0 {
//                 // The LHR with the most hits is treated as the "canonical" LHR
//                 // Hits on any other LHR reduce the homogeniety confidence,
//                 //
//                 most_hits = lhr.hit_count as u32;
//             } else {
//                 other_hits = other_hits.saturating_add(lhr.hit_count as u32);
//             }
//         }
//         let interpretation = if most_hits == 0 && other_hits == 0 {
//             NotEnoughData
//         } else if other_hits > 0 {
//             // most_hits > other_hits due to sorting
//             let most_hits_magnitude = most_hits.checked_ilog2().expect("most_hits to be positive"); // ≤ 32 due to i32
//             if most_hits_magnitude > 7 /* ^= 128 */ && other_hits < most_hits_magnitude {
//                 Homogenous
//             } else {
//                 Heterogenous
//             }
//         } else {
//             // Since other_hits == 0 and most_hits > other_hits, most_hits > 0
//             Homogenous
//         };
//         let score = match interpretation {
//             NotEnoughData => 0.0,
//             Homogenous => {}
//         };
//         // FIXME confidence; continue computation
//         Ok((SubnetInterpretation::Homogenous, 0))
//     }
// }
