use ipnet::Ipv6Net;
use prefix_crab::prefix_split;

use crate::{analyse::context, prefix_tree::ContextOps};

use super::recommend::{ReProbePriority, SplitRecommendation};

/// percent rating, i.e. between 0 and 100
pub type Confidence = u8;

pub const MAX_CONFIDENCE: Confidence = 100;

pub fn rate(context: &context::Context, rec: &SplitRecommendation) -> Confidence {
    use SplitRecommendation::*;

    let net = context.node().net;
    match rec {
        YesSplit { priority } => rate_yes(priority, &net),
        NoKeep { priority } => rate_no(priority, &net),
        CannotDetermine { priority } => rate_no(priority, &net),
    }
}

fn rate_yes(prio: &ReProbePriority, net: &Ipv6Net) -> Confidence {
    // TODO evaluate what different function we could use for the "distinct responses required for same" case,
    // it seems like it should be different.
    rate_no(prio, net)
}

fn rate_no(prio: &ReProbePriority, net: &Ipv6Net) -> Confidence {
    let thresh = min_equivalent_responses_thresh(net);
    debug_assert!(thresh > 0);
    let evidence = 0i32.max(prio.supporting_observations) as u32;
    if evidence > thresh {
        MAX_CONFIDENCE
    } else {
        evidence
            .saturating_mul(MAX_CONFIDENCE as u32)
            .div_euclid(thresh) // ^= discard remainder
            .try_into()
            .expect("division where a < b to be less than one (thus result < 100 < 255)")
    }
}

// /16
const MIN_REALISTIC_AGGREGATE: u8 = 16;

// chosen based on the number of rounds needed to reach the threshold
// for a /64 and scaling exp. for larger nets
const THRESH_FOR_64_CONST: u32 = (prefix_split::SAMPLES_PER_SUBNET as u32) * 4u32;

fn min_equivalent_responses_thresh(net: &Ipv6Net) -> u32 {
    // https://docs.google.com/spreadsheets/d/1rOlf3MNCSIj58b9yB1Ni-Dnr2sWrostZoqOcSjIm_To/edit#gid=0
    let prefix_len_capped = MIN_REALISTIC_AGGREGATE.max(net.prefix_len());
    let depth = 64u8.saturating_sub(prefix_len_capped);
    let min_response_exp = 2u32.saturating_pow(depth as u32);
    min_response_exp.saturating_mul(THRESH_FOR_64_CONST)
}
