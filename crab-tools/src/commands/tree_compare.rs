use std::{collections::HashSet, path::PathBuf};

use anyhow::*;
use clap::Args;
use db_model::{
    analyse::Confidence,
    persist::{self, dsl::CidrMethods, ConfidenceLoader, DieselErrorFixCause},
    prefix_tree::{AsNumber, LhrSetHash, MergeStatus, PriorityClass},
};
use diesel::prelude::*;
use futures::executor;
use ipnet::{IpNet, Ipv6Net};
use itertools::Itertools;
use log::{info, warn};
use prefix_crab::helpers::{ip::ExpectV6, stop::flatten};
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

    #[arg(long, env = "DATABASE_URL_REF")]
    db_url_ref: String,

    target_prefix: Ipv6Net,
    out_file: PathBuf,
}

impl Params {
    fn reference_db(&self) -> persist::Params {
        persist::Params::new(self.db_url_ref.to_owned())
    }
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
pub struct ComparedNode {
    pub net: Ipv6Net,
    pub asn: AsNumber,
    pub net_len: u8,

    pub eval_present: NodePresence,
    pub eval_confidence: Option<Confidence>,
    pub eval_class: Option<PriorityClass>,
    pub eval_lhrs: Option<String>,

    pub ref_present: NodePresence,
    pub ref_confidence: Option<Confidence>,
    pub ref_class: Option<PriorityClass>,
    pub ref_lhrs: Option<String>,

    pub compare_presence: PresenceRelation,
    pub compare_confidence: i32,
}

impl ComparedNode {
    fn new(prefix: Option<Prefix>, ref_prefix: Option<Prefix>) -> Self {
        let present_prefix = prefix
            .or(ref_prefix)
            .expect("at least one side to have a prefix (otherwise pointless)");

        let eval_present: NodePresence = prefix.into();
        let eval_confidence = prefix.map(|it| it.confidence);
        let ref_present = ref_prefix.into();
        let ref_confidence = ref_prefix.map(|it| it.confidence);

        let compare_presence = eval_present.compare_to(&ref_present);

        Self {
            net: present_prefix.net,
            asn: present_prefix.asn,
            net_len: present_prefix.net.prefix_len(),

            eval_present,
            eval_confidence,
            eval_class: prefix.map(|it| it.priority_class),
            eval_lhrs: prefix.map(|it| it.lhr_set_hash.to_string()[0..10].to_string()),

            ref_present,
            ref_confidence,
            ref_class: ref_prefix.map(|it| it.priority_class),
            ref_lhrs: ref_prefix.map(|it| it.lhr_set_hash.to_string()[0..10].to_string()),

            compare_presence,
            compare_confidence: eval_confidence.unwrap_or(0) as i32
                - ref_confidence.unwrap_or(0) as i32,
        }
    }
}

#[derive(Serialize, PartialEq, Eq)]
pub enum NodePresence {
    Missing,
    KeptLeaf,
    KeepCandidate,
    SplitNode,
    SplitCandidate,
}

impl From<Option<Prefix>> for NodePresence {
    fn from(value: Option<Prefix>) -> Self {
        if let Some(pfx) = value {
            match pfx.merge_status {
                MergeStatus::Leaf | MergeStatus::UnsplitRoot => {
                    match suggests_split(pfx.priority_class) {
                        Some(true) => Self::SplitCandidate,
                        Some(false) => {
                            if pfx.confidence == 255 {
                                Self::KeptLeaf
                            } else {
                                Self::KeepCandidate
                            }
                        }
                        None => Self::KeepCandidate, // inaccurate for HighFresh, but that could be filtered if it's an issue
                    }
                }
                MergeStatus::MinSizeReached => Self::KeptLeaf,
                MergeStatus::SplitDown => Self::SplitNode,
                MergeStatus::MergedUp => Self::Missing, // for simplicity .. could also have special reverted status
                MergeStatus::SplitRoot => Self::SplitNode,
                MergeStatus::Blocked => Self::Missing,
            }
        } else {
            Self::Missing
        }
    }
}

fn suggests_split(priority_class: PriorityClass) -> Option<bool> {
    use PriorityClass as P;
    Some(match priority_class {
        P::HighFresh => return None,
        P::HighOverlapping => true,
        P::HighDisjoint => true,
        P::MediumSameMulti => true,
        P::MediumSameRatio => false,
        P::MediumSameMany => false,
        P::MediumSameSingle => false,
        P::MediumMultiWeird => true,
        P::LowWeird => false,
        P::LowUnknown => return None,
    })
}

impl NodePresence {
    fn is_split_coded(&self) -> bool {
        matches!(self, Self::SplitCandidate | Self::SplitNode)
    }

    fn is_candidate(&self) -> bool {
        matches!(self, Self::SplitCandidate | Self::KeepCandidate)
    }

    fn compare_to(&self, other: &NodePresence) -> PresenceRelation {
        // self is considered to be evaluated

        if other == self {
            PresenceRelation::Same
        } else if self == &Self::Missing {
            PresenceRelation::ReferenceOnly
        } else if other == &Self::Missing {
            PresenceRelation::EvalOnly
        } else if self.is_split_coded() && other.is_split_coded() {
            if self.is_candidate() || other.is_candidate() {
                PresenceRelation::CandidateSame
            } else {
                PresenceRelation::Same
            }
        } else {
            if self.is_candidate() || other.is_candidate() {
                PresenceRelation::CandidateDifferent
            } else {
                PresenceRelation::Different
            }
        }
    }
}

#[derive(Serialize)]
pub enum PresenceRelation {
    Same,
    CandidateSame,
    Different,
    CandidateDifferent,
    EvalOnly,
    ReferenceOnly,
}

async fn write(out_file: File, mut res_rx: Receiver<ComparedNode>) -> Result<()> {
    let mut writer = csv_async::AsyncSerializer::from_writer(out_file);

    while let Some(next) = res_rx.recv().await {
        writer.serialize(next).await?;
    }

    info!("Sender closed result channel.");
    Ok(())
}

async fn run(params: Params, res_tx: Sender<ComparedNode>) -> Result<()> {
    info!("Loading nodes..");

    let prefixes = select_prefixes(&params.target_prefix, &params.persist)?;

    let mut seen_prefixes_eval: HashSet<Ipv6Net> = Default::default();
    for prefix in prefixes.iter() {
        seen_prefixes_eval.insert(prefix.net);
    }

    analyse_all_parallel(params.clone(), res_tx.clone(), prefixes.into_iter(), false).await?;

    let ref_prefixes = select_prefixes(&params.target_prefix, &params.reference_db())?;
    let ref_prefixes = ref_prefixes
        .into_iter()
        .filter(|ref_prefix| !seen_prefixes_eval.contains(&ref_prefix.net));

    analyse_all_parallel(params, res_tx, ref_prefixes, true).await?;

    Ok(())
}

async fn analyse_all_parallel<I: Iterator<Item = Prefix>>(
    params: Params,
    res_tx: Sender<ComparedNode>,
    mut prefixes: I,
    swap: bool,
) -> Result<()> {
    info!("Launching parallel analysis.");

    let mut futures = JoinSet::new();

    for _ in 0..20 {
        if let Some(prefix) = prefixes.next() {
            futures.spawn(analyse_one(prefix, params.clone(), swap));
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
                    futures.spawn(analyse_one(prefix, params.clone(), swap));
                } else {
                    info!("Out of nets to schedule. Waiting for the rest to complete.");
                }
            }
            Err(e) => {
                warn!(" !!! Error during analysis {:?}. Continuing.", e);
            }
        }
    }

    info!("Analysis finished.");
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct Prefix {
    net: Ipv6Net,
    asn: AsNumber,
    merge_status: MergeStatus,
    confidence: Confidence,
    priority_class: PriorityClass,
    lhr_set_hash: LhrSetHash,
}

type PrefixLoader = (
    IpNet,
    AsNumber,
    MergeStatus,
    ConfidenceLoader,
    PriorityClass,
    LhrSetHash,
);

impl From<PrefixLoader> for Prefix {
    fn from(
        (net, asn, merge_status, confidence, priority_class, lhr_set_hash): PrefixLoader,
    ) -> Self {
        Self {
            net: net.expect_v6(),
            merge_status,
            confidence: confidence.into(),
            asn,
            priority_class,
            lhr_set_hash,
        }
    }
}

fn select_prefixes(root: &Ipv6Net, db_params: &persist::Params) -> Result<Vec<Prefix>> {
    use db_model::schema::prefix_tree::dsl::*;
    let mut conn = persist::connect_manual("crab-tools - tree-compare - select", db_params)?;

    let raw_nets: Vec<PrefixLoader> = prefix_tree
        .filter(net.subnet_or_eq6(root))
        .select((
            net,
            asn,
            merge_status,
            confidence,
            priority_class,
            lhr_set_hash,
        ))
        .load(&mut conn)
        .fix_cause()?;

    Ok(raw_nets.into_iter().map_into().collect_vec())
}

async fn analyse_one(prefix: Prefix, params: Params, swap: bool) -> Result<ComparedNode> {
    analyse_one_inner(prefix, params, swap)
        .await
        .with_context(|| anyhow!("analysing prefix {:?}", prefix))
}

async fn analyse_one_inner(prefix: Prefix, params: Params, swap: bool) -> Result<ComparedNode> {
    use db_model::schema::prefix_tree::dsl::*;

    let db_params = if swap {
        params.reference_db()
    } else {
        params.persist
    };

    let mut ref_conn = persist::connect_manual("crab-tools - tree-compare - ref", &db_params)?;

    let ref_prefix: Option<PrefixLoader> = prefix_tree
        .filter(net.eq6(&prefix.net))
        .select((
            net,
            asn,
            merge_status,
            confidence,
            priority_class,
            lhr_set_hash,
        ))
        .get_result(&mut ref_conn)
        .optional()
        .fix_cause()?;

    let mut ref_prefix = ref_prefix.map(Into::into);
    let mut eval_prefix = Some(prefix);

    if swap {
        std::mem::swap(&mut ref_prefix, &mut eval_prefix);
    }

    Ok(ComparedNode::new(ref_prefix, eval_prefix))
}
