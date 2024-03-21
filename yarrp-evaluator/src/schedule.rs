use core::num;
use std::collections::HashSet;
use std::fs::File;
use std::net::Ipv6Addr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use ipnet::{Ipv6Net, PrefixLenError};
use itertools::Itertools;
use log::{error, info, trace, warn};
use nohash_hasher::IntSet;
use prefix_crab::{blocklist, prefix_split};
use queue_models::probe_response::TraceResult;
use serde::Serialize;
use tokio::sync::mpsc::{Receiver, UnboundedSender};
use tokio_stream::wrappers::ReceiverStream;

use crate::schedule::model::EvaluateRequest;
use crate::schedule::task::SchedulerTask;
use crate::yarrp_call;

pub use self::model::ProbeResponse;

pub mod model;
mod task;

#[derive(Args, Clone)]
#[group(id = "scheduler")]
pub struct Params {
    #[clap(flatten)]
    base: yarrp_call::Params,

    #[clap(flatten)]
    blocklist: blocklist::Params,

    target_net: Ipv6Net,

    out_file: PathBuf,
}

pub const PROBES_PER_NET: u16 = 16;
pub const SUBNET_SIZE: u8 = 48;

#[derive(Serialize)]
pub struct SubnetRow {
    pub subnet: Ipv6Net,
    pub received_count: u64,
    pub last_hop_routers: Vec<Ipv6Addr>,
}

struct Scheduler {
    params: Params,
}

pub async fn run(params: Params) -> Result<()> {
    Scheduler { params }.run().await
}

impl Scheduler {
    async fn run(&mut self) -> Result<()> {
        self.check_preflight().await?;
        let out_file = File::create_new(self.params.out_file.to_owned())?;

        info!(
            "Scheduling for {} on {} granularity...",
            self.params.target_net, SUBNET_SIZE
        );
        let mut task = SchedulerTask::new(self.params.clone())?;
        let mut num_subnets = 0i64;

        for net in self.params.target_net.subnets(SUBNET_SIZE)? {
            let sample = prefix_split::sample_single_net(&net, PROBES_PER_NET);
            let req = EvaluateRequest {
                net,
                targets: sample.addresses,
            };
            task.push_work(req)?;
            num_subnets += 1;
        }
        info!("Pushed {} subnets", num_subnets);

        let results = task.run().await?;
        let mut writer = csv::Writer::from_writer(out_file);

        for response in results {
            let mut received_count = 0u64;
            let mut last_hop_routers = HashSet::<Ipv6Addr>::new();

            for result in response.results {
                match result {
                    TraceResult::LastResponsiveHop(hop) => {
                        received_count += 1;
                        last_hop_routers.insert(hop.last_hop_addr);
                    }
                    TraceResult::NoResponse { target_addr: _ } => {}
                }
            }

            let row = SubnetRow {
                subnet: response.net,
                received_count,
                last_hop_routers: last_hop_routers.into_iter().collect_vec(),
            };

            writer.serialize(row)?;
        }
        Ok(())
    }

    async fn check_preflight(&self) -> Result<()> {
        let params = self.params.clone();
        tokio::task::spawn_blocking(move || {
            params
                .base
                .to_caller_verifying_sudo()?
                .verify_sudo_access()?;
            Ok::<(), anyhow::Error>(())
        })
        .await
        .context("pre-flight sudo access check failed")??;
        info!(
            "Loading blocklist from `{:?}`",
            params.blocklist.blocklist_file
        );
        blocklist::read(params.blocklist)
            .context("pre-flight blocklist check failed")
            .map(|_| ())
    }
}
