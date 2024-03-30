use ipnet::Ipv6Net;
use prefix_crab::confidence_threshold;

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
    rate_with_thresh(prio, confidence_threshold::split_distinct_responses_thresh(net))
}

fn rate_no(prio: &ReProbePriority, net: &Ipv6Net) -> Confidence {
    rate_with_thresh(prio, confidence_threshold::keep_equivalent_responses_thresh(net))
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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::*;
    use db_model::prefix_tree::PriorityClass::MediumSameMulti;
    use db_model::test_utils::*;

    // "Google Sheet cases" are based on /confidence_threshold.ods

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
