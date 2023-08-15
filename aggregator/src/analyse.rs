pub use model::*;
pub use result::*;

pub mod context;
pub mod echo;
pub mod map64;
pub mod model;
pub mod persist;
pub mod result;
pub mod split {
    use std::array::from_fn;

    use anyhow::{Context, Result};
    use diesel::{prelude::*, PgConnection, QueryDsl, SelectableHelper};
    use ipnet::IpNet;
    use log::warn;
    use prefix_crab::prefix_split::{self, PrefixSplit, SplitSubnet};

    use crate::{
        persist::dsl::CidrMethods,
        prefix_tree::ContextOps,
        schema::measurement_tree::{dsl::measurement_tree, target_net},
    };

    use super::{context, MeasurementTree};

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
}
