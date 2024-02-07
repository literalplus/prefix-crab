use db_model::prefix_tree::AsNumber;
use nohash_hasher::IntMap;
use tracing::instrument;

use crate::schedule::Params;

#[instrument(name = "allocate AS budgets", skip(params))]
pub fn allocate(params: &Params) -> AsBudgets {
    AsBudgets::new(params.analysis_timer_max_prefix_per_as)
}

#[derive(Default)]
pub struct AsBudgets {
    allocation_per_as: usize,
    consumed_per_as: IntMap<AsNumber, usize>,
    exhausted_asns: Vec<AsNumber>,
}

impl AsBudgets {
    fn new(allocation_per_as: usize) -> Self {
        Self {
            allocation_per_as,
            ..Default::default()
        }
    }

    pub fn try_consume(&mut self, asn: AsNumber) -> bool {
        let consumed = self.consumed_per_as.entry(asn).or_default();
        if consumed >= &mut self.allocation_per_as {
            false
        } else {
            *consumed += 1;
            if consumed == &mut self.allocation_per_as {
                self.exhausted_asns.push(asn)
            }
            true
        }
    }

    pub fn has_exhausted_asns(&self) -> bool {
        !self.exhausted_asns.is_empty()
    }

    pub fn get_exhausted_asns(&self) -> &[AsNumber] {
        &self.exhausted_asns
    }
}
