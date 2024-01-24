use log::trace;

use crate::analyse::{HitCount, WeirdItem};
use db_model::prefix_tree::PriorityClass;

use super::subnet::{LhrDiff, Subnets};

/// Changes in the split algorithm are versioned to allow us to invalidate results of an older version
/// if we find out that it is flawed.
pub const ALGO_VERSION: i32 = 120;

#[derive(Debug, Eq, PartialEq)]
pub enum SplitRecommendation {
    /// Two different subnets have been detected & a split is suggested
    YesSplit { priority: ReProbePriority },
    /// The two subnets look similar & a split is not suggested.
    NoKeep { priority: ReProbePriority },
    /// An action could not be determined
    CannotDetermine { priority: ReProbePriority },
}

impl<'a> SplitRecommendation {
    pub fn priority(&'a self) -> &'a ReProbePriority {
        match self {
            SplitRecommendation::YesSplit { priority } => priority,
            SplitRecommendation::NoKeep { priority } => priority,
            SplitRecommendation::CannotDetermine { priority } => priority,
        }
    }

    pub fn should_split(&self) -> bool {
        matches!(self, Self::YesSplit { priority: _ })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ReProbePriority {
    pub class: PriorityClass,
    pub supporting_observations: HitCount,
}

pub fn recommend(subnets: &Subnets) -> SplitRecommendation {
    use super::subnet::Diff as D;
    use PriorityClass as P;
    use SplitRecommendation as R;

    let diff = subnets.lhr_diff();
    trace!("LHR diff: {:?}", diff);
    match diff {
        D::BothNone => recommend_without_lhr_data(subnets),
        D::BothSameSingle { shared } => R::NoKeep {
            priority: ReProbePriority {
                class: P::MediumSameSingle,
                supporting_observations: shared.total_hit_count(),
            },
        },
        D::BothSameMultiple { shared } => rate_same_multi(shared),
        D::OverlappingOrDisjoint { shared, distinct } => R::YesSplit {
            priority: ReProbePriority {
                class: if shared.is_empty() {
                    P::HighDisjoint
                } else {
                    P::HighOverlapping
                },
                supporting_observations: sum_deranking_most_popular(shared)
                    + sum_lhr_hits(distinct), // supporting the observation that there is more than one LHR a) where the overall set is the same and b) the sets are distinct
            },
        },
    }
}

fn rate_same_multi(shared: Vec<LhrDiff>) -> SplitRecommendation {
    use PriorityClass as P;
    use SplitRecommendation as R;

    // What same-multi is trying to accomplish is to detect cases where both subnets are divided into parts
    // that are each served by the same routing infrastructure (no prefix-local routers) and thus have the same
    // LHR set, but would result in significant grouping if split further.
    //
    // However, if the allocated ratio is the same in both subnets, the ratio would be the same either way.
    // We have now reduced to a much softer version that really only splits in extreme cases, because there
    // have been so many problems with too aggressive splits, and not really a good case that what we are trying to
    // check for here is even really very relevant at all.
    //
    // A possible smarter expansion would be to, if a split is suggested with high confidence (increase the bound
    // for this prio class specifically, to group together the LHR sets and figure out in advance if, with the
    // current data, there is even a somewhat contiguous group of recursive-subnets that has only one of the LHRs
    // and is significantly different from the rest.

    if shared.len() >= 5 {
        // if we have too many LHRs, a ratio is no longer really meaningful and if both subnets have the same
        // LHR set then most likely they are equivalent.
        return R::NoKeep {
            priority: ReProbePriority {
                class: P::MediumSameMany,
                // supporting the observation that there are five or more LHRs
                // ignoring the five most popular ones is a bit much -> impractical
                // so just apply a general 75% buff (more LHRs might show up if we keep probing)
                supporting_observations: sum_lhr_hits(shared).div_euclid(4),
            },
        };
    }

    let total_per_subnet: Vec<HitCount> = (0..2usize)
        .map(|i| shared.iter().map(|it| it.hit_counts[i]).sum())
        .collect();

    // We allow more leeway for the percent difference.
    // Rejecting a split isn't a huge problem since we usually get many attempts to split but not many to merge/revert.
    let thresh = 15; // % difference is allowed

    let mut ratio_is_same = true;
    for diff in shared.iter() {
        let [left, right] = diff.hit_counts;
        let both_significant = left > 3 && right > 3;
        let (left, right) = (left * 100, right * 100);
        let (left, right) = (
            left.saturating_div(total_per_subnet[0]),
            right.saturating_div(total_per_subnet[1]),
        );

        let pct_diff = left.abs_diff(right);
        if pct_diff > thresh && both_significant { // don't allow 3 or fewer hits to reject the ratio of the entire prefix
            ratio_is_same = false;
            break;
        }
    }

    if ratio_is_same {
        R::NoKeep {
            priority: ReProbePriority {
                class: P::MediumSameRatio,
                // supporting the observation that the ratio is the same
                supporting_observations: sum_lhr_hits(shared),
            },
        }
    } else {
        R::YesSplit {
            priority: ReProbePriority {
                class: P::MediumSameMulti,
                // supporting the observation that there is more than one LHR
                supporting_observations: sum_deranking_most_popular(shared),
            },
        }
    }
}

fn sum_deranking_most_popular(lhrs: Vec<LhrDiff>) -> HitCount {
    let most_popular_hits = lhrs
        .iter()
        .map(|it| it.total_hit_count())
        .max()
        .unwrap_or(0);
    lhrs.into_iter()
        .map(|lhr| {
            if lhr.total_hit_count() == most_popular_hits {
                // The most popular router counts less because if there were only hits to it, the
                // conclusion would be that there should be no split
                lhr.total_hit_count().div_euclid(2)
            } else {
                lhr.total_hit_count()
            }
        })
        .sum()
}

fn sum_lhr_hits(lhrs: Vec<LhrDiff>) -> HitCount {
    lhrs.into_iter().map(|it| it.total_hit_count()).sum()
}

fn recommend_without_lhr_data(subnets: &Subnets) -> SplitRecommendation {
    use super::subnet::Diff::*;
    use PriorityClass::*;
    use SplitRecommendation::*;

    let diff = subnets.weird_diff();
    trace!("Weirdness diff: {:?}", diff);
    match diff {
        BothNone => CannotDetermine {
            priority: ReProbePriority {
                class: LowUnknown,
                supporting_observations: subnets.sum_subtrees(|t| t.unresponsive_count),
            },
        },
        BothSameSingle { shared } => NoKeep {
            priority: ReProbePriority {
                class: LowWeird,
                supporting_observations: shared.hit_count,
            },
        },
        BothSameMultiple { shared } => CannotDetermine {
            // TODO what should we do in this case, especially if it keeps being like this?
            // e.g. check ratio, or perform more analyses deeper into the tree, or group by /64 and see if the pattern can be split
            priority: ReProbePriority {
                class: MediumMultiWeird,
                supporting_observations: sum_weird_hits(shared),
            },
        },
        OverlappingOrDisjoint {
            shared: _,
            distinct,
        } => YesSplit {
            priority: ReProbePriority {
                class: MediumMultiWeird,
                supporting_observations: sum_weird_hits(distinct), // supporting the observation that the weirdness signatures are different
            },
        },
    }
}

fn sum_weird_hits(weirds: Vec<WeirdItem>) -> HitCount {
    weirds.into_iter().map(|it| it.hit_count).sum()
}

#[cfg(test)]
mod tests {
    use assertor::{assert_that, EqualityAssertion};

    use super::{PriorityClass::*, SplitRecommendation::*, *};
    use crate::analyse::{split::subnet::Subnets, MeasurementTree};
    use db_model::test_utils::*;

    #[test]
    fn same_single_lhr() {
        // given
        let measurements = vec![
            gen_tree_with_lhr_101(TREE_LEFT_NET, 2),
            gen_tree_with_lhr_101(TREE_RIGHT_NET, 7),
        ];

        // when
        let rec = when_recommend(measurements);

        // then
        assert_that!(rec).is_equal_to(NoKeep {
            priority: ReProbePriority {
                class: MediumSameSingle,
                supporting_observations: 9,
            },
        })
    }

    fn when_recommend(measurements: Vec<MeasurementTree>) -> SplitRecommendation {
        recommend(&Subnets::new(net(TREE_BASE_NET), &measurements).unwrap())
    }

    #[test]
    fn same_multi_lhr_different_ratio() {
        // given
        let mut measurements = vec![
            gen_tree_with_lhr_101(TREE_LEFT_NET, 13),  // 61.9%
            gen_tree_with_lhr_101(TREE_RIGHT_NET, 31), // 79.4%
        ];
        for measurement in &mut measurements {
            gen_add_lhr_beef(measurement, 8);
        }

        // when
        let rec = when_recommend(measurements);

        // then
        assert_that!(rec).is_equal_to(YesSplit {
            priority: ReProbePriority {
                class: MediumSameMulti,
                supporting_observations: 38, // (13+31)/2 + 2*8
            },
        })
    }

    #[test]
    fn same_multi_lhr_same_many() {
        // given
        let mut measurements = vec![
            gen_tree_with_lhr_101(TREE_LEFT_NET, 12),
            gen_tree_with_lhr_101(TREE_RIGHT_NET, 500), // ratio doesn't matter here!
        ];
        let lhrs = &[
            "2001:db8:baba::1",
            "2001:db8:baba::2",
            "2001:db8:baba::3",
            "2001:db8:baba::4",
        ];
        for lhr in lhrs {
            for tree in measurements.iter_mut() {
                gen_add_lhr(tree, lhr, 4);
            }
        }

        // when
        let rec = when_recommend(measurements);

        // then
        assert_that!(rec).is_equal_to(NoKeep {
            priority: ReProbePriority {
                class: MediumSameMany,
                supporting_observations: 136, // all divided by 4
            },
        })
    }

    #[test]
    fn same_multi_lhr_same_ratio_few_hits_pos() {
        // given
        let mut measurements = vec![
            gen_tree_with_lhr_101(TREE_LEFT_NET, 100),  // 50%
            gen_tree_with_lhr_101(TREE_RIGHT_NET, 170), // 62%
        ];
        gen_add_lhr_beef(&mut measurements[0], 100); // 50%
        gen_add_lhr_beef(&mut measurements[1], 104); // 38%

        // when
        let rec = when_recommend(measurements);

        // then
        assert_that!(rec).is_equal_to(NoKeep {
            priority: ReProbePriority {
                class: MediumSameRatio,
                supporting_observations: 474, // all
            },
        })
    }

    #[test]
    fn same_multi_lhr_same_ratio_swapped() {
        // given
        let mut measurements = vec![
            gen_tree_with_lhr_101(TREE_LEFT_NET, 104),
            gen_tree_with_lhr_101(TREE_RIGHT_NET, 100),
        ];
        for measurement in &mut measurements {
            gen_add_lhr_beef(measurement, 100);
        }

        // when
        let rec = when_recommend(measurements);

        // then
        assert_that!(rec).is_equal_to(NoKeep {
            priority: ReProbePriority {
                class: MediumSameRatio,
                supporting_observations: 404, // all
            },
        })
    }

    #[test]
    fn same_multi_lhr_same_ratio_many_hits_pos() {
        // given
        let mut measurements = vec![
            gen_tree_with_lhr_101(TREE_LEFT_NET, 211387), // 94% (but 42.5% of prefix total)
            gen_tree_with_lhr_101(TREE_RIGHT_NET, 259860), // 95.1% (but 52% of prefix total)
        ];
        gen_add_lhr_beef(&mut measurements[0], 12576);
        gen_add_lhr_beef(&mut measurements[1], 13342);

        // when
        let rec = when_recommend(measurements);

        // then
        assert_that!(rec).is_equal_to(NoKeep {
            priority: ReProbePriority {
                class: MediumSameRatio,
                supporting_observations: 497165, // all
            },
        })
    }

    #[test]
    fn same_multi_lhr_same_ratio_many_hits_neg() {
        // given
        let mut measurements = vec![
            gen_tree_with_lhr_101(TREE_LEFT_NET, 211387),  // 94%
            gen_tree_with_lhr_101(TREE_RIGHT_NET, 283860), // just over 95.5%
        ];
        gen_add_lhr_beef(&mut measurements[0], 12576);
        gen_add_lhr_beef(&mut measurements[1], 13342);

        // when
        let rec = when_recommend(measurements);

        // then
        assert_that!(rec).is_equal_to(NoKeep {
            priority: ReProbePriority {
                class: MediumSameRatio,
                supporting_observations: 521165, // all
            },
        })
    }

    #[test]
    fn same_multi_lhr_same_ratio_abs_percent() {
        // given
        let mut measurements = vec![
            gen_tree_with_lhr_101(TREE_LEFT_NET, 151),
            gen_tree_with_lhr_101(TREE_RIGHT_NET, 159),
        ];
        gen_add_lhr_beef(&mut measurements[0], 2672);
        gen_add_lhr_beef(&mut measurements[1], 2834);
        // note that these aren't in the 5% per absolute numbers, but in relative numbers

        // when
        let rec = when_recommend(measurements);

        // then
        assert_that!(rec).is_equal_to(NoKeep {
            priority: ReProbePriority {
                class: MediumSameRatio,
                supporting_observations: 5816, // all
            },
        })
    }

    #[test]
    fn overlapping() {
        // given
        let mut measurements = vec![
            gen_tree_with_lhr_101(TREE_LEFT_NET, 2),
            gen_tree_with_lhr_101(TREE_RIGHT_NET, 3),
        ];
        gen_add_lhr_beef(&mut measurements[0], 12);

        // when
        let rec = when_recommend(measurements);

        // then
        assert_that!(rec).is_equal_to(YesSplit {
            priority: ReProbePriority {
                class: HighOverlapping,
                supporting_observations: 14, // 5/2 + 12
            },
        })
    }

    #[test]
    fn disjoint() {
        // given
        let measurements = vec![
            gen_tree_with_lhr_101(TREE_LEFT_NET, 2),
            gen_tree_with_lhr_beef(TREE_RIGHT_NET, 3),
            gen_tree_with_lhr_beef(TREE_RIGHT_NET_ALT, 3),
        ];

        // when
        let rec = when_recommend(measurements);

        // then
        assert_that!(rec).is_equal_to(YesSplit {
            priority: ReProbePriority {
                class: HighDisjoint,
                supporting_observations: 8,
            },
        })
    }
}
