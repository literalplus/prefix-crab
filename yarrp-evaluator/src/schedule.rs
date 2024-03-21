use std::collections::HashSet;
use std::fs::File;
use std::net::Ipv6Addr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use ipnet::Ipv6Net;
use itertools::Itertools;
use log::info;

use prefix_crab::{blocklist, prefix_split};
use queue_models::probe_response::TraceResult;
use serde::Serialize;

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

    #[arg(long, default_value = "48")]
    subnet_size: u8,

    target_net: Ipv6Net,

    out_file: PathBuf,
}

pub const PROBES_PER_NET: u16 = 16;

#[derive(Serialize)]
pub struct SubnetRow {
    pub subnet: Ipv6Net,
    pub received_count: u64,
    pub last_hop_routers: String,
}

impl SubnetRow {
    fn new<T: IntoIterator<Item = Ipv6Addr>>(
        subnet: Ipv6Net,
        received_count: u64,
        lhrs_raw: T,
    ) -> Self {
        let last_hop_routers = lhrs_raw
            .into_iter()
            .map(|addr| format!("{}", addr))
            .join(";");
        Self {
            subnet,
            received_count,
            last_hop_routers,
        }
    }
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
        let out_file = File::create_new(&self.params.out_file)?;

        info!(
            "Scheduling at {} pps for {} on {} granularity...",
            self.params.base.rate_pps, self.params.target_net, self.params.subnet_size,
        );
        let mut task = SchedulerTask::new(self.params.clone())?;
        let mut num_subnets = 0i64;

        for net in self.params.target_net.subnets(self.params.subnet_size)? {
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
        info!("Received {} results", results.len());

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

            let row = SubnetRow::new(response.net, received_count, last_hop_routers);

            writer.serialize(row)?;
        }

        info!("Done writing.");
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
