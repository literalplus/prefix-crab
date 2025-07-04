use std::path::PathBuf;

use anyhow::*;
use clap::Args;
use db_model::{
    analyse::{Confidence, HitCount, SplitAnalysis, SplitAnalysisResult},
    persist::{
        self,
        dsl::{masklen, CidrMethods},
        ConfidenceLoader, DieselErrorFixCause,
    },
    prefix_tree::{AsNumber, LhrSetHash, MergeStatus, PriorityClass},
};
use diesel::{dsl::not, prelude::*};
use futures::executor;
use ipnet::{IpNet, Ipv6Net};
use itertools::Itertools;
use log::{debug, info, warn};
use prefix_crab::{
    confidence_threshold,
    helpers::{ip::ExpectV6, stop::flatten},
};
use serde::Serialize;
use tokio::{
    fs::File,
    sync::mpsc::{self, Receiver, Sender},
    task::JoinSet,
    try_join,
};

#[derive(Args, Clone)]
pub struct Params {
    #[clap(flatten)]
    persist: persist::Params,

    target_prefix: Ipv6Net,
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

#[derive(Serialize)]
pub struct EdgeAnalysis {
    pub net: Ipv6Net,
    pub asn: AsNumber,
    pub net_len: u8,
    pub merge_status: MergeStatus,
    pub confidence: Confidence,
    pub lhr_set_hash: String,
    pub is_eligible_for_split: bool,

    pub run_count: usize,
    pub run_len_avg: f64,

    pub last_run: PriorityClass,
    pub last_run_len: u32,
    pub last_run_start_evidence: HitCount,
    pub last_run_end_evidence: HitCount,
    pub last_run_target_evidence: u32,
    pub last_run_should_split: Option<bool>,
}

impl EdgeAnalysis {
    fn new(prefix: Prefix, runs: Vec<Run>) -> Self {
        let Prefix {
            net,
            asn,
            merge_status,
            confidence,
            lhr_set_hash,
        } = prefix;
        let run_count = runs.len();
        let last_run = runs.iter().last().expect("at least one run");
        Self {
            net,
            asn,
            net_len: net.prefix_len(),
            merge_status,
            confidence,
            lhr_set_hash: lhr_set_hash.to_string()[0..10].to_string(),
            is_eligible_for_split: merge_status.is_eligible_for_split(),

            run_count,
            run_len_avg: runs.iter().map_into::<f64>().sum::<f64>() / (run_count as f64),

            last_run: last_run.prio,
            last_run_len: last_run.len,
            last_run_start_evidence: last_run.start_evidence,
            last_run_end_evidence: last_run.end_evidence,
            last_run_should_split: last_run.should_split,
            last_run_target_evidence: last_run.target_evidence(&net),
        }
    }
}

impl From<&Run> for f64 {
    fn from(value: &Run) -> Self {
        value.len.into()
    }
}

#[derive(Clone)]
pub struct Run {
    pub prio: PriorityClass,
    pub len: u32,
    pub start_evidence: HitCount,
    pub end_evidence: HitCount,
    pub should_split: Option<bool>,
}

impl Run {
    fn target_evidence(&self, net: &Ipv6Net) -> u32 {
        if self.should_split.unwrap_or(false) {
            confidence_threshold::split_distinct_responses_thresh(net)
        } else {
            confidence_threshold::keep_equivalent_responses_thresh(net)
        }
    }
}

async fn write(out_file: File, mut res_rx: Receiver<EdgeAnalysis>) -> Result<()> {
    let mut writer = csv_async::AsyncSerializer::from_writer(out_file);

    while let Some(next) = res_rx.recv().await {
        writer.serialize(next).await?;
    }

    info!("Sender closed result channel.");
    Ok(())
}

async fn run(params: Params, res_tx: Sender<EdgeAnalysis>) -> Result<()> {
    persist::initialize(&params.persist)?;
    info!("Loading nodes..");

    let mut prefixes = select_prefixes(&params.target_prefix)?.into_iter();
    let mut futures = JoinSet::new();

    for _ in 0..20 {
        if let Some(prefix) = prefixes.next() {
            futures.spawn(analyse_one(prefix));
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

                if let Some(prefix) = prefixes.next() {
                    futures.spawn(analyse_one(prefix));
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

#[derive(Clone, Copy, Debug)]
struct Prefix {
    net: Ipv6Net,
    merge_status: MergeStatus,
    confidence: Confidence,
    asn: AsNumber,
    lhr_set_hash: LhrSetHash,
}

type PrefixLoader = (IpNet, MergeStatus, ConfidenceLoader, AsNumber, LhrSetHash);

impl From<PrefixLoader> for Prefix {
    fn from((net, merge_status, confidence, asn, lhr_set_hash): PrefixLoader) -> Self {
        Self {
            net: net.expect_v6(),
            merge_status,
            confidence: confidence.into(),
            asn,
            lhr_set_hash,
        }
    }
}

fn select_prefixes(root: &Ipv6Net) -> Result<Vec<Prefix>> {
    use db_model::schema::prefix_tree::dsl::*;
    let mut conn = persist::connect("crab-tools - edge-analyse - init")?;

    let raw_nets: Vec<PrefixLoader> = prefix_tree
        .filter(net.subnet_or_eq6(root))
        .filter(masklen(net).lt(64)) // /64 nets are not analysed further
        .select((net, merge_status, confidence, asn, lhr_set_hash))
        .load(&mut conn)
        .fix_cause()?;

    Ok(raw_nets.into_iter().map_into().collect_vec())
}

async fn analyse_one(prefix: Prefix) -> Result<EdgeAnalysis> {
    analyse_one_inner(prefix)
        .await
        .with_context(|| anyhow!("analysing prefix {:?}", prefix))
}

async fn analyse_one_inner(prefix: Prefix) -> Result<EdgeAnalysis> {
    use db_model::schema::split_analysis::dsl::*;

    let mut conn = persist::connect("crab-tools - edge-analyse - job")?;

    let analyses: Vec<SplitAnalysis> = split_analysis
        .filter(tree_net.eq6(&prefix.net))
        .filter(not(completed_at.is_null()))
        .order(completed_at.asc())
        .load(&mut conn)
        .fix_cause()?;
    let mut analyses = analyses.into_iter();

    debug!(
        "Loaded {} completed analyses for net {}",
        analyses.len(),
        &prefix.net,
    );

    let mut last_run: Run = if let Some(analysis) = analyses.next() {
        analysis
            .result
            .ok_or_else(|| anyhow!("no result for completed first analysis {}", analysis.id))?
            .into()
    } else {
        return Err(anyhow!("No completed analyses for {}", &prefix.net));
    };

    let mut runs = vec![];

    for analysis in analyses {
        let res = analysis
            .result
            .ok_or_else(|| anyhow!("no result for completed analysis {}", analysis.id))?;

        if last_run.prio == res.class {
            last_run.len += 1;
            last_run.end_evidence = res.evidence;
        } else {
            let prev_run = std::mem::replace(&mut last_run, res.into());
            runs.push(prev_run);
        }
    }

    runs.push(last_run);

    Ok(EdgeAnalysis::new(prefix, runs))
}

impl From<SplitAnalysisResult> for Run {
    fn from(value: SplitAnalysisResult) -> Self {
        Run {
            prio: value.class,
            len: 1,
            start_evidence: value.evidence,
            end_evidence: value.evidence,
            should_split: value.should_split,
        }
    }
}
