pub use model::*;
pub use result::*;

pub mod context;
pub mod echo;
pub mod map64;
pub mod model;
pub mod persist;
pub mod result;
pub mod split {
    use std::{array::from_fn, cmp::Reverse, net::Ipv6Addr};

    use anyhow::{Context, Result};
    use diesel::{prelude::*, PgConnection, QueryDsl, SelectableHelper};
    use ipnet::IpNet;
    use itertools::Itertools;
    use log::warn;
    use prefix_crab::prefix_split::{self, PrefixSplit, SplitSubnet};

    use crate::{
        persist::dsl::CidrMethods,
        prefix_tree::ContextOps,
        schema::measurement_tree::{dsl::measurement_tree, target_net},
    };

    use super::{context, LhrItem, MeasurementTree};

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
        let net_to_analyse = request.node().try_net_into_v6()?;
        let split = prefix_split::split(net_to_analyse)?;
        let subnets = AnalysisSubnet::from_split(split, relevant_measurements);
        Ok(())
    }

    struct AnalysisSubnet {
        pub subnet: SplitSubnet,
        pub trees: Vec<MeasurementTree>,
    }

    impl AnalysisSubnet {
        pub fn from_split(
            split: PrefixSplit,
            relevant_measurements: Vec<MeasurementTree>,
        ) -> [AnalysisSubnet; 2] {
            let mut result = split.into_subnets().map(|subnet| Self {
                subnet,
                trees: vec![],
            });
            let split_nets: [IpNet; 2] = from_fn(|i| IpNet::V6(*&result[i].subnet.network));
            for tree in relevant_measurements {
                let mut moved = false;
                for (i, subnet) in result.iter_mut().enumerate() {
                    if &tree.target_net <= &split_nets[i] {
                        subnet.trees.push(tree);
                        moved = true;
                        break;
                    }
                }
                if !moved {
                    warn!(
                        "Received a tree from DB that didn't fit into either subnet: {:?}",
                        split_nets
                    );
                }
            }
            result
        }
    }

    enum SubnetInterpretation {
        Homogenous,
        Heterogenous,
        NotEnoughData,
    }

    impl TryFrom<AnalysisSubnet> for (SubnetInterpretation, i32) {
        type Error = anyhow::Error;

        fn try_from(value: AnalysisSubnet) -> Result<Self> {
            let mut tree = MeasurementTree::empty(value.subnet.network);
            for found_tree in value.trees {
                tree.consume_merge(found_tree)?;
            }
            let lhrs_most_to_least_hits = tree
                .last_hop_routers
                .items
                .iter()
                .sorted_unstable_by_key(|(addr, item)| Reverse(item.hit_count));
            let mut most_hits = 0u32;
            let mut other_hits = 0u32;
            for (_, lhr) in lhrs_most_to_least_hits {
                if most_hits == 0 {
                    // The LHR with the most hits is treated as the "canonical" LHR
                    // Hits on any other LHR reduce the homogeniety confidence,
                    //
                    most_hits = lhr.hit_count as u32;
                } else {
                    other_hits = other_hits.saturating_add(lhr.hit_count as u32);
                }
            }
            let interpretation = if most_hits == 0 && other_hits == 0 {
                SubnetInterpretation::NotEnoughData
            } else if other_hits > 0 {
                // most_hits > other_hits due to sorting
                let most_hits_magnitude =
                    most_hits.checked_ilog2().expect("most_hits to be positive"); // ≤ 32 due to i32
                if most_hits_magnitude > 7 /* ^= 128 */ && other_hits < most_hits_magnitude {
                    SubnetInterpretation::Homogenous
                } else {
                    SubnetInterpretation::Heterogenous
                }
            } else { // Since other_hits == 0 and most_hits > other_hits, most_hits > 0
                SubnetInterpretation::Homogenous
            };
            let score = match interpretation {
                _ => todo!()
            };
            // FIXME confidence; continue computation
            Ok((SubnetInterpretation::Homogenous, 0))
        }
    }
}
