use anyhow::*;
use diesel::dsl::{count, exists, not, now, IntervalDsl};
use diesel::sql_types::Integer;
use ipnet::{IpNet, Ipv6Net};
use log::debug;
use prefix_crab::helpers::ip::ExpectAllV6;
use rand::Rng;
use std::collections::{btree_map, BTreeMap};

use diesel::prelude::*;
use diesel::{PgConnection, QueryDsl};

use crate::persist::DieselErrorFixCause;
use crate::prefix_tree::{MergeStatus, PriorityClass}; 

#[derive(Default)]
pub struct ClassBudgets {
    available_per_class: BTreeMap<PriorityClass, u64>,
    allocated_per_class: BTreeMap<PriorityClass, u32>,
}

impl ClassBudgets {
    fn new(available_per_class: BTreeMap<PriorityClass, u64>) -> Self {
        Self {
            available_per_class,
            allocated_per_class: BTreeMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.allocated_per_class.is_empty()
    }
}

pub fn allocate(conn: &mut PgConnection, total_prefixes: u32) -> Result<ClassBudgets> {
    let available_per_class = count_available_per_class(conn)?;
    if available_per_class.is_empty() {
        debug!("There are no leaves to schedule a prefix analysis for");
        return Ok(ClassBudgets::default());
    }

    let mut budgets = ClassBudgets::new(available_per_class);

    for _ in 0..total_prefixes {
        if !budgets.allocate_next() {
            break;
        }
    }
    Ok(budgets)
}

macro_rules! leaf_where_no_analysis {
    (let $var_name:ident = it) => {
        use crate::schema::prefix_tree::dsl::*;
        use crate::schema::split_analysis::dsl as ana;

        let a_pending_analysis = ana::split_analysis
            .select(0.into_sql::<Integer>())
            .filter(ana::tree_net.eq(net))
            .filter(ana::completed_at.is_null())
            .filter(not(ana::created_at.lt(now - 2.days()))); // retry unfinished analyses

        let $var_name = prefix_tree
            .filter(not(exists(a_pending_analysis)))
            .filter(merge_status.eq(MergeStatus::Leaf));
    };
}

fn count_available_per_class(conn: &mut PgConnection) -> Result<BTreeMap<PriorityClass, u64>> {
    leaf_where_no_analysis!(let base = it);
    let tuples = base
        .group_by(priority_class)
        .select((priority_class, count(priority_class)))
        .load::<(PriorityClass, i64)>(conn)
        .fix_cause()?;

    Ok(tuples
        .into_iter()
        .map(|(prio, count_signed)| (prio, count_signed as u64))
        .collect())
}

impl ClassBudgets {
    fn allocate_next(&mut self) -> bool {
        let remaining_ratio: u16 = self
            .available_per_class
            .keys()
            .map(|prio| allocation_ratio(prio))
            .sum();
        if remaining_ratio == 0 {
            return false;
        }

        let random_choice = rand::thread_rng().gen_range(1..=remaining_ratio);
        self.allocate_by_ratio(random_choice);

        true
    }

    fn allocate_by_ratio(&mut self, choice: u16) {
        let chosen = self
            .choose_by_ratio(choice)
            .expect("a priority class to be chosen");
        *self.allocated_per_class.entry(chosen).or_default() += 1;
        let available = self
            .available_per_class
            .get_mut(&chosen)
            .expect("chosen class to be available");
        if available <= &mut 1 {
            self.available_per_class.remove(&chosen);
        } else {
            *available -= 1;
        }
    }

    fn choose_by_ratio(&self, choice: u16) -> Option<PriorityClass> {
        let mut accumulator = 0u16;
        for prio in self.available_per_class.keys() {
            let ratio = allocation_ratio(prio);
            accumulator += ratio;
            if accumulator >= choice {
                return Some(*prio);
            }
        }
        None
    }
}

fn allocation_ratio(class: &PriorityClass) -> u16 {
    // function to ensure exhaustiveness
    use PriorityClass as P;

    match class {
        P::HighFresh => 25,
        P::HighOverlapping => 13,
        P::HighDisjoint => 12,
        P::MediumSameMulti => 23,
        P::MediumSameSingle => 13,
        P::MediumMultiWeird => 10,
        P::LowWeird => 2,
        P::LowUnknown => 2,
    }
}

pub struct BudgetsIntoIter {
    delegate: btree_map::IntoIter<PriorityClass, u32>,
    available_reduced_by_allocations: BTreeMap<PriorityClass, u64>,
}

impl IntoIterator for ClassBudgets {
    type Item = ClassBudget;

    type IntoIter = BudgetsIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        BudgetsIntoIter {
            delegate: self.allocated_per_class.into_iter(),
            available_reduced_by_allocations: self.available_per_class,
        }
    }
}

impl Iterator for BudgetsIntoIter {
    type Item = ClassBudget;

    fn next(&mut self) -> Option<Self::Item> {
        let (class, allocated) = self.delegate.next()?;
        let available = *self
            .available_reduced_by_allocations
            .entry(class)
            .or_default()
            + (allocated as u64);
        Some(ClassBudget {
            class,
            allocated,
            available,
        })
    }
}

pub struct ClassBudget {
    pub class: PriorityClass,
    allocated: u32,
    available: u64, // TODO use for tablesample
}

impl ClassBudget {
    pub fn select_prefixes(&self, conn: &mut PgConnection) -> Result<Vec<Ipv6Net>> {
        leaf_where_no_analysis!(let base = it);

        let raw_nets: Vec<IpNet> = base
            .filter(priority_class.eq(self.class))
            .limit(self.allocated as i64)
            .select(net)
            .load(conn)
            .fix_cause()?;

        Ok(raw_nets.expect_all_v6())
    }
}

#[cfg(test)]
mod tests {
    use assertor::{assert_that, EqualityAssertion, OptionAssertion};

    use super::*;

    #[test]
    fn allocate_choice() {
        // Note: This test relies on the deterministic iteration order of a BTreeMap (as opposed to e.g. HashMap)

        // given
        let cases = vec![
            (1, PriorityClass::HighFresh),
            (16, PriorityClass::HighFresh),
            (25, PriorityClass::HighFresh),
            (26, PriorityClass::LowUnknown),
            (27, PriorityClass::LowUnknown),
        ];

        // when
        for (choice, expected) in cases {
            let budgets = ClassBudgets::new(given_availables());

            // when
            let chosen = budgets.choose_by_ratio(choice);

            // then
            assert_that!(chosen).is_some();
            assert_that!(chosen.unwrap()).is_equal_to(expected);
        }
    }

    fn given_availables() -> BTreeMap<PriorityClass, u64> {
        vec![
            (PriorityClass::LowUnknown, 56),
            (PriorityClass::HighFresh, 2),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn allocate_allocation() {
        // given
        let mut budgets = ClassBudgets::new(given_availables());

        // when
        budgets.allocate_by_ratio(27);

        // then

        assert_that!(budgets.allocated_per_class[&PriorityClass::LowUnknown]).is_equal_to(1);
        assert_that!(budgets.allocated_per_class.len()).is_equal_to(1);
        assert_that!(budgets.available_per_class[&PriorityClass::LowUnknown]).is_equal_to(55);
    }

    #[test]
    fn allocate_now_unavailable() {
        // given
        let mut budgets = ClassBudgets::new(given_availables());

        // when
        budgets.allocate_by_ratio(1);
        budgets.allocate_by_ratio(1);
        budgets.allocate_by_ratio(1);

        // then

        assert_that!(budgets.allocated_per_class[&PriorityClass::HighFresh]).is_equal_to(2);
        assert_that!(budgets.allocated_per_class[&PriorityClass::LowUnknown]).is_equal_to(1);
        assert_that!(budgets.allocated_per_class.len()).is_equal_to(2);
        assert_that!(budgets.available_per_class.get(&PriorityClass::HighFresh)).is_none();
        assert_that!(budgets.available_per_class.get(&PriorityClass::LowUnknown)).is_some();
    }
}
