use ipnet::Ipv6Net;
use prefix_crab::prefix_split;

use super::recommend::{ReProbePriority, SplitRecommendation};
use db_model::analyse::*;

pub fn rate(net: Ipv6Net, rec: &SplitRecommendation) -> Confidence {
    use SplitRecommendation as R;

    match rec {
        R::YesSplit { priority } => rate_yes(priority, &net),
        R::NoKeep { priority } => rate_no(priority, &net),
        R::CannotDetermine { priority } => rate_no(priority, &net),
    }
}

fn rate_yes(prio: &ReProbePriority, net: &Ipv6Net) -> Confidence {
    rate_with_thresh(prio, max_equivalent_responses_thresh(net))
}

fn rate_no(prio: &ReProbePriority, net: &Ipv6Net) -> Confidence {
    rate_with_thresh(prio, min_equivalent_responses_thresh(net))
}

fn rate_with_thresh(prio: &ReProbePriority, thresh: u32) -> Confidence {
    debug_assert!(thresh > 0);
    let evidence = 0i32.max(prio.supporting_observations) as u32;

    evidence
        .saturating_mul(CONFIDENCE_THRESH as u32)
        .div_euclid(thresh) // ^= discard remainder
        .try_into()
        .unwrap_or(Confidence::MAX)
}

// /16
const MIN_REALISTIC_AGGREGATE: u8 = 16;

// chosen based on the number of rounds needed to reach the threshold
// for a /64 and scaling exp. for larger nets
const THRESH_FOR_64_KEEP: u32 = (prefix_split::SAMPLES_PER_SUBNET as u32) * 4u32;

// higher since a split is not reversible atm, but using less aggressive exponential growth
const THRESH_FOR_64_SPLIT: u32 = (prefix_split::SAMPLES_PER_SUBNET as u32) * 16u32;

fn min_equivalent_responses_thresh(net: &Ipv6Net) -> u32 {
    // https://docs.google.com/spreadsheets/d/1rOlf3MNCSIj58b9yB1Ni-Dnr2sWrostZoqOcSjIm_To/edit#gid=0
    let prefix_len_capped = MIN_REALISTIC_AGGREGATE.max(net.prefix_len());
    let height = 64u8.saturating_sub(prefix_len_capped);
    let min_response_exp = 2f64.powf(height as f64 / 4f64);
    debug_assert!(min_response_exp >= 1.0);
    (min_response_exp * (THRESH_FOR_64_KEEP as f64)).trunc() as u32
}

fn max_equivalent_responses_thresh(net: &Ipv6Net) -> u32 {
    // https://docs.google.com/spreadsheets/d/1rOlf3MNCSIj58b9yB1Ni-Dnr2sWrostZoqOcSjIm_To/edit#gid=0
    let prefix_len_capped = MIN_REALISTIC_AGGREGATE.max(net.prefix_len());
    let height = 64u8.saturating_sub(prefix_len_capped);
    let min_response_exp = 1.4f64.powf(height as f64 / 4f64);
    debug_assert!(min_response_exp >= 1.0);
    (min_response_exp * (THRESH_FOR_64_SPLIT as f64)).trunc() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::*;
    use db_model::prefix_tree::PriorityClass::MediumSameMulti;
    use db_model::test_utils::*;

    // "Google Sheet cases" are based on https://docs.google.com/spreadsheets/d/1rOlf3MNCSIj58b9yB1Ni-Dnr2sWrostZoqOcSjIm_To/edit#gid=164692237

    #[test]
    fn min_responses_google_sheet_cases() -> Result<()> {
        // given
        let cases = [
            (64, 64),
            (63, 76),
            (62, 90),
            (61, 107),
            (60, 128),
            (16, 262_144),
            (12, 262_144),
        ];

        // when, then
        for (prefix_size, threshold) in cases {
            let net = Ipv6Net::new(addr(TREE_LHR_BEEF), prefix_size).unwrap();
            let actual = min_equivalent_responses_thresh(&net);
            if actual != threshold {
                bail!(
                    "Prefix size {}: Expected threshold {} but got {}",
                    prefix_size,
                    threshold,
                    actual
                );
            }
        }
        Ok(())
    }

    #[test]
    fn max_responses_google_sheet_cases() -> Result<()> {
        // given
        let cases = [
            (64, 256),
            (63, 278),
            (62, 302),
            (61, 329),
            (60, 358),
            (16, 14513),
            (12, 14513),
        ];

        // when, then
        for (prefix_size, threshold) in cases {
            let net = Ipv6Net::new(addr(TREE_LHR_BEEF), prefix_size).unwrap();
            let actual = max_equivalent_responses_thresh(&net);
            if actual != threshold {
                bail!(
                    "Prefix size {}: Expected threshold {} but got {}",
                    prefix_size,
                    threshold,
                    actual
                );
            }
        }
        Ok(())
    }

    #[test]
    fn keep_responses_confidence_google_sheet() -> Result<()> {
        // given
        let cases = [
            (64, 4, 6),
            (64, 32, 50),
            (64, 64, 100),
            (64, 678123, 255),
            (24, 32768, 50),
        ];

        // when, then
        for (prefix_size, evidence, expected) in cases {
            let rec = SplitRecommendation::NoKeep {
                priority: ReProbePriority {
                    class: MediumSameMulti,
                    supporting_observations: evidence,
                },
            };
            let net = Ipv6Net::new(addr(TREE_LHR_BEEF), prefix_size).unwrap();
            let actual = rate(net, &rec);
            if actual != expected {
                bail!(
                    "Prefix size {}: Expected confidence {}% but got {}%",
                    prefix_size,
                    expected,
                    actual
                );
            }
        }
        Ok(())
    }
}
