use ipnet::Ipv6Net;

use crate::prefix_split;

// /16
pub const MIN_REALISTIC_AGGREGATE: u8 = 16;

// chosen based on the number of rounds needed to reach the threshold
// for a /64 and scaling exp. for larger nets
pub const THRESH_FOR_64_KEEP: u32 = (prefix_split::SAMPLES_PER_SUBNET as u32) * 4u32;

// higher since a split is not reversible atm, but using less aggressive exponential growth
pub const THRESH_FOR_64_SPLIT: u32 = (prefix_split::SAMPLES_PER_SUBNET as u32) * 16u32;

pub fn keep_equivalent_responses_thresh(net: &Ipv6Net) -> u32 {
    // See: /confidence_threshold.ods -> Sheet "Equivalent"
    let prefix_len_capped = MIN_REALISTIC_AGGREGATE.max(net.prefix_len());
    let height = 64u8.saturating_sub(prefix_len_capped);
    let min_response_exp = 2f64.powf(height as f64 / 4f64);
    debug_assert!(min_response_exp >= 1.0);
    (min_response_exp * (THRESH_FOR_64_KEEP as f64)).trunc() as u32
}

pub fn split_distinct_responses_thresh(net: &Ipv6Net) -> u32 {
    // See: /confidence_threshold.ods -> Sheet "Distinct"
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

    // "Google Sheet cases" are based on /confidence_threshold.ods

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
            let net = format!("2001:db8:beef::20/{}", prefix_size).parse().unwrap();
            let actual = keep_equivalent_responses_thresh(&net);
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
            let net = format!("2001:db8:beef::20/{}", prefix_size).parse().unwrap();
            let actual = split_distinct_responses_thresh(&net);
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
}
