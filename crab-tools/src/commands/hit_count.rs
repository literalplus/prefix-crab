use std::{
    ops::{Add, AddAssign},
    path::PathBuf,
};

use anyhow::*;
use clap::Args;
use db_model::persist::{self, dsl::CidrMethods, DieselErrorFixCause};
use diesel::{dsl::not, prelude::*};
use futures::executor;
use ipnet::{IpNet, Ipv6Net};
use itertools::Itertools;
use lazy_static::lazy_static;
use log::{debug, info, warn};
use prefix_crab::helpers::{ip::ExpectV6, stop::flatten};
use queue_models::probe_response::{
    EchoProbeResponse, ResponseKey, TraceResponse,
    TraceResult::{LastResponsiveHop, NoResponse},
};
use serde::Serialize;
use serde_json::Value;
use tokio::{
    fs::File,
    sync::mpsc::{self, Receiver, Sender},
    task::JoinSet,
    try_join,
};
lazy_static! {
    static ref SUBNET_IGNORE_LIST: [IpNet; 8] = [
        // avoid counting them twice...
        "2001:890:c000::/34".parse().unwrap(), // contained in AS8447
        "2001:628:453::/48".parse().unwrap(),  // contained in AS1853
        "2001:628:2000::/48".parse().unwrap(), // contained in AS1853
        "2a03:3180:f::/48".parse().unwrap(),   // contained in AS44453
        "2a01:aea0:df3::/48".parse().unwrap(), // contained in AS42473
        "2a01:aea0:df4::/47".parse().unwrap(), // contained in AS42473
        "2a01:aea0:dd4::/47".parse().unwrap(), // contained in AS42473
        "2a01:aea0:dd3::/48".parse().unwrap(), // contained in AS42473
    ];
}

#[derive(Args, Clone)]
pub struct Params {
    #[clap(flatten)]
    persist: persist::Params,

    out_file: PathBuf,
}

pub fn handle(params: Params) -> Result<()> {
    let (res_tx, res_rx) = mpsc::channel(512);

    let out_file = std::fs::File::create_new(&params.out_file)?;

    let analyse_handle = tokio::spawn(run(params, res_tx));
    let write_handle = tokio::spawn(write(File::from_std(out_file), res_rx));

    executor::block_on(async {
        try_join!(flatten(analyse_handle), flatten(write_handle))?;
        Ok(())
    })?;

    Ok(())
}

#[derive(Debug, Default, Serialize)]
pub struct HitSummary {
    pub net: Ipv6Net,

    pub zmap_sent: usize,
    pub zmap_received_echo: usize,
    pub zmap_received_err: usize,

    pub yarrp_sent: usize,
    pub yarrp_missed: usize,
    pub yarrp_in_prefix: usize,
}

impl HitSummary {
    fn new(net: Ipv6Net) -> Self {
        Self {
            net,
            ..Default::default()
        }
    }
}

impl AddAssign<&HitSummary> for HitSummary {
    fn add_assign(&mut self, rhs: &HitSummary) {
        self.zmap_sent += rhs.zmap_sent;
        self.zmap_received_echo += rhs.zmap_received_echo;
        self.zmap_received_err += rhs.zmap_received_err;
        self.yarrp_sent += rhs.yarrp_sent;
        self.yarrp_missed += rhs.yarrp_missed;
        self.yarrp_in_prefix += rhs.yarrp_in_prefix;
    }
}

async fn write(out_file: File, mut res_rx: Receiver<HitSummary>) -> Result<()> {
    let mut writer = csv_async::AsyncSerializer::from_writer(out_file);
    let mut summary = HitSummary::new(Ipv6Net::default());

    while let Some(next) = res_rx.recv().await {
        summary += &next;
        writer.serialize(next).await?;

        info!("Aggregation status: {:?}", summary);
    }

    info!("Summary: {:?}", summary);

    info!("Sender closed result channel.");
    Ok(())
}

async fn run(params: Params, res_tx: Sender<HitSummary>) -> Result<()> {
    persist::initialize(&params.persist)?;
    info!("Loading prefixes..");

    let mut prefixes = select_prefixes()?.into_iter();
    let mut futures = JoinSet::new();

    for _ in 0..20 {
        if let Some(net) = prefixes.next() {
            futures.spawn(analyse_one(net));
        } else {
            info!("Didn't even get 20 start prefixes to analyse.");
            break;
        }
    }

    info!("Started 20 prefix analyses in parallel.");

    while let Some(result) = futures.join_next().await {
        let result = result?; // join error
        match result {
            Result::Ok(analysis) => {
                info!(" ... Analysed {}", analysis.net);
                res_tx.send(analysis).await?;

                if let Some(net) = prefixes.next() {
                    futures.spawn(analyse_one(net));
                } else {
                    info!("Out of nets to schedule. Waiting for the rest to complete.");
                }
            }
            Err(e) => {
                warn!(" !!! Error during analysis {:?}. Continuing.", e);
            }
        }
    }

    Ok(())
}

fn select_prefixes() -> Result<Vec<Ipv6Net>> {
    use db_model::schema::as_prefix::dsl::*;
    let mut conn = persist::connect("crab-tools - hit-count - init")?;

    let raw_nets: Vec<IpNet> = as_prefix
        .filter(not(net.eq_any(SUBNET_IGNORE_LIST.iter())))
        .filter(deleted.ne(true))
        .select(net)
        .load(&mut conn)
        .fix_cause()?;

    Ok(raw_nets.into_iter().map(|it| it.expect_v6()).collect_vec())
}

async fn analyse_one(net: Ipv6Net) -> Result<HitSummary> {
    analyse_one_inner(net)
        .await
        .with_context(|| anyhow!("analysing net {}", net))
}

async fn analyse_one_inner(net: Ipv6Net) -> Result<HitSummary> {
    use db_model::schema::response_archive::dsl::*;

    let mut conn = persist::connect("crab-tools - hit-count - job")?;

    let responses: Vec<Value> = response_archive
        .select(data)
        .filter(path.subnet_or_eq6(&net))
        .load(&mut conn)
        .fix_cause()?;

    debug!(
        "Loaded {} archived responses for net {}",
        responses.len(),
        net
    );

    let mut result = HitSummary::new(net);

    for response in responses {
        if let Result::Ok(zmap) = serde_json::from_value::<EchoProbeResponse>(response.clone()) {
            for split in zmap.splits {
                for res in split.responses {
                    result.zmap_sent += res.intended_targets.len();
                    if matches!(
                        res.key,
                        ResponseKey::DestinationUnreachable { kind: _, from: _ }
                            | ResponseKey::TimeExceeded { from: _ }
                    ) {
                        result.zmap_received_err += res.intended_targets.len();
                    } else if matches!(res.key, ResponseKey::EchoReply { different_from: _ }) {
                        result.zmap_received_echo += res.intended_targets.len();
                    }
                }
            }
        } else if let Result::Ok(yarrp) = serde_json::from_value::<TraceResponse>(response.clone())
        {
            result.yarrp_sent += yarrp.results.len();
            for target in yarrp.results {
                match target {
                    LastResponsiveHop(hop) => {
                        if net.contains(&hop.last_hop_addr) {
                            result.yarrp_in_prefix += 1;
                        }
                    }
                    NoResponse { target_addr: _ } => {
                        result.yarrp_missed += 1;
                    }
                }
            }
        } else {
            warn!("Unparseable response archive entry {:?}", response);
        }
    }

    Ok(result)
}
