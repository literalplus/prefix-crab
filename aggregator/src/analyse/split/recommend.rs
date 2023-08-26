use itertools::Itertools;
use log::trace;

use crate::analyse::{HitCount, LhrItem, WeirdItem};

use super::subnet::Subnets;

/// Changes in the split algorithm are versioned to allow us to invalidate results of an older version
/// if we find out that it is flawed.
pub const ALGO_VERSION: i32 = 100;

#[derive(Debug, Eq, PartialEq)]
pub enum SplitRecommendation {
    /// Two different subnets have been detected & a split is suggested
    YesSplit { priority: ReProbePriority },
    /// The two subnets look similar & a split is not suggested.
    NoKeep { priority: ReProbePriority },
    /// An action could not be determined
    CannotDetermine { priority: ReProbePriority },
}

#[derive(Debug, Eq, PartialEq)]
pub struct ReProbePriority {
    class: PriorityClass,
    supporting_observations: HitCount,
}

#[derive(Debug, Eq, PartialEq)]
pub enum PriorityClass {
    Low,
    MediumLow,
    Medium,
    MediumHigh,
    High,
}

pub fn recommend(subnets: Subnets) -> SplitRecommendation {
    use super::subnet::Diff::*;
    use PriorityClass::*;
    use SplitRecommendation::*;

    let diff = subnets.lhr_diff();
    trace!("LHR diff: {:?}", diff);
    match diff {
        BothNone => recommend_without_lhr_data(subnets),
        BothSameSingle { shared } => NoKeep {
            priority: ReProbePriority {
                class: Medium,
                supporting_observations: shared.hit_count,
            },
        },
        BothSameMultiple { shared } => YesSplit {
            priority: ReProbePriority {
                class: MediumHigh,
                supporting_observations: sum_except_most_popular(shared), // supporting the observation that there is more than one LHR
            },
        },
        OverlappingOrDisjoint { shared, distinct } => YesSplit {
            priority: ReProbePriority {
                class: High,
                supporting_observations: sum_except_most_popular(shared) + sum_lhr_hits(distinct), // supporting the observation that there is more than one LHR a) where the overall set is the same and b) the sets are distinct
            },
        },
    }
}

fn sum_except_most_popular(lhrs: Vec<LhrItem>) -> HitCount {
    lhrs.into_iter()
        .map(|it| it.hit_count)
        .sorted_unstable()
        .skip(1)
        .sum()
}

fn sum_lhr_hits(lhrs: Vec<LhrItem>) -> HitCount {
    lhrs.into_iter().map(|it| it.hit_count).sum()
}

fn recommend_without_lhr_data(subnets: Subnets) -> SplitRecommendation {
    use super::subnet::Diff::*;
    use PriorityClass::*;
    use SplitRecommendation::*;

    let diff = subnets.weird_diff();
    trace!("Weirdness diff: {:?}", diff);
    match diff {
        BothNone => CannotDetermine {
            priority: ReProbePriority {
                class: Low,
                supporting_observations: subnets.sum_subtrees(|t| t.unresponsive_count),
            },
        },
        BothSameSingle { shared } => NoKeep {
            priority: ReProbePriority {
                class: Low,
                supporting_observations: shared.hit_count,
            },
        },
        BothSameMultiple { shared } => CannotDetermine {
            // TODO what should we do in this case, especially if it keeps being like this?
            // e.g. check ratio, or perform more analyses deeper into the tree, or group by /64 and see if the pattern can be split
            priority: ReProbePriority {
                class: MediumLow,
                supporting_observations: sum_weird_hits(shared),
            },
        },
        OverlappingOrDisjoint {
            shared: _,
            distinct,
        } => YesSplit {
            priority: ReProbePriority {
                class: Low,
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

    use super::{*, PriorityClass::*, SplitRecommendation::*};
    use crate::{
        analyse::{split::subnet::Subnets, MeasurementTree},
        test_utils::*,
    };

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
                class: Medium,
                supporting_observations: 9,
            },
        })
    }

    fn when_recommend(measurements: Vec<MeasurementTree>) -> SplitRecommendation {
        recommend(Subnets::new(net(TREE_BASE_NET), measurements).unwrap())
    }

    #[test]
    fn same_multi_lhr() {
        // given
        let mut measurements = vec![
            gen_tree_with_lhr_101(TREE_LEFT_NET, 2),
            gen_tree_with_lhr_101(TREE_RIGHT_NET, 3),
        ];
        for measurement in &mut measurements {
            gen_add_lhr_beef(measurement, 4);
        }

        // when
        let rec = when_recommend(measurements);

        // then
        assert_that!(rec).is_equal_to(YesSplit{
            priority: ReProbePriority {
                class: MediumHigh,
                supporting_observations: 8,
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
        assert_that!(rec).is_equal_to(YesSplit{
            priority: ReProbePriority {
                class: High,
                supporting_observations: 12,
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
        assert_that!(rec).is_equal_to(YesSplit{
            priority: ReProbePriority {
                class: High,
                supporting_observations: 8,
            },
        })
    }
}
