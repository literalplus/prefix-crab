use std::{
    collections::HashMap,
    ops::{Index, IndexMut},
};

use prefix_crab::prefix_split::NetIndex;

use crate::analyse::CanFollowUp;

use super::{Split, SplitAnalysis};

#[derive(Debug)]
pub struct SplitAnalysisDetails {
    pub analysis: SplitAnalysis,
    splits: Vec<Split>,
}

impl SplitAnalysisDetails {
    pub fn new(analysis: SplitAnalysis, existing_splits: Vec<Split>) -> Self {
        let mut map = HashMap::<NetIndex, Split>::with_capacity(NetIndex::value_count() as usize);
        for existing_split in existing_splits {
            if let Ok(valid_index) = <NetIndex>::try_from(existing_split.net_index) {
                map.insert(valid_index, existing_split);
            }
        }
        for index in NetIndex::iter_values() {
            map.entry(index)
                .or_insert_with(|| Split::new(&analysis, index));
        }
        let splits = map.into_values().collect();
        Self { analysis, splits }
    }

    pub fn borrow_splits(&self) -> &Vec<Split> {
        &self.splits
    }
}

impl Index<&NetIndex> for SplitAnalysisDetails {
    type Output = Split;

    /// This should never fail under normal circumstances due to pre-filling of
    /// splits up to [NetIndex::count_values] at construction time.
    fn index(&self, index: &NetIndex) -> &Self::Output {
        &self.splits[<usize>::from(*index)]
    }
}

impl IndexMut<&NetIndex> for SplitAnalysisDetails {
    /// This should never fail under normal circumstances due to pre-filling of
    /// splits up to [NetIndex::count_values] at construction time.
    fn index_mut(&mut self, index: &NetIndex) -> &mut Self::Output {
        &mut self.splits[<usize>::from(*index)]
    }
}

impl CanFollowUp for SplitAnalysisDetails {
    fn needs_follow_up(&self) -> bool {
        self.splits.iter().any(|it| it.data.needs_follow_up())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use assertor::*;
    use chrono::NaiveDateTime;

    use crate::analyse::Stage;

    use super::*;

    #[test]
    fn fill_splits() {
        // given
        let analysis = SplitAnalysis {
            id: 0i64,
            tree_id: 0i64,
            created_at: NaiveDateTime::default(),
            completed_at: None,
            stage: Stage::PendingTrace,
            split_prefix_len: 2,
        };
        let my_index = NetIndex::try_from(0u8).unwrap();
        let mut my_split = Split::new(&analysis, my_index);
        let my_weirdness = "henlo";
        my_split
            .data
            .weird_behaviours
            .insert(my_weirdness.to_string());
        let existing = vec![my_split];
        // when
        let created = SplitAnalysisDetails::new(analysis, existing);
        // then
        assert_that!(created.splits).has_length(NetIndex::value_count() as usize);
        let mut expected = HashSet::<String>::new();
        expected.insert(my_weirdness.to_string());
        assert_that!(created[&my_index].data.weird_behaviours).is_equal_to(expected);
    }
}
