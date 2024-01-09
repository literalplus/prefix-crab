use anyhow::{Context, Result};
use diesel::dsl::now;
use diesel::{prelude::*, PgConnection};
use itertools::Itertools;
use log::warn;
use prefix_crab::blocklist::PrefixBlocklist;

use crate::analyse::split::subnet::Subnet;
use crate::analyse::SplitAnalysis;
use crate::persist::dsl::CidrMethods;
use crate::persist::DieselErrorFixCause;
use db_model::prefix_tree::{ContextOps, LhrSetHash, MergeStatus, PrefixTree, PriorityClass};

use super::recommend::{self, ReProbePriority, SplitRecommendation};

use super::subnet::Subnets;
use super::{context, Confidence, SplitAnalysisResult};

pub fn save_recommendation(
    conn: &mut PgConnection,
    context: &context::Context,
    recommendation: &SplitRecommendation,
    confidence: Confidence,
    lhr_set_hash: LhrSetHash,
) -> Result<()> {
    SaveRecommendation {
        context,
        recommendation,
        confidence,
        lhr_set_hash,
    }
    .save(conn)
}

struct SaveRecommendation<'a, 'b> {
    pub context: &'a context::Context,
    pub recommendation: &'b SplitRecommendation,
    pub confidence: Confidence,
    lhr_set_hash: LhrSetHash,
}

impl<'a, 'b> SaveRecommendation<'a, 'b> {
    fn save(self, conn: &mut PgConnection) -> Result<()> {
        match self.save_to_analysis(conn)? {
            1 => {}
            count => warn!(
                "Unexpected update count {} for analysis {:?}",
                count,
                self.analysis().id()
            ),
        }
        match self.save_to_prefix(conn)? {
            1 => {}
            count => warn!(
                "Unexpected update count {} for prefix {:?}",
                count,
                &self.context.log_id()
            ),
        }
        Ok(())
    }

    fn save_to_analysis(&self, conn: &mut PgConnection) -> Result<usize> {
        use crate::schema::split_analysis::dsl::*;

        let rec_model: Option<SplitAnalysisResult> = Some(self.recommendation.into());
        diesel::update(self.analysis())
            .set((result.eq(rec_model), completed_at.eq(now)))
            .execute(conn)
            .fix_cause()
            .with_context(|| {
                format!(
                    "saving recommendation {:?} for analysis {}",
                    self.recommendation,
                    self.analysis().id()
                )
            })
    }

    fn analysis(&self) -> &SplitAnalysis {
        &self.context.analysis
    }

    fn save_to_prefix(&self, conn: &mut PgConnection) -> Result<usize> {
        diesel::update(self.context.node())
            .set(self.as_prefix_changeset())
            .execute(conn)
            .fix_cause()
            .with_context(|| {
                format!(
                    "saving recommendation {:?} for prefix {}",
                    self.recommendation,
                    self.context.log_id()
                )
            })
    }

    fn as_prefix_changeset(&self) -> PrefixRecommendationChangeset {
        PrefixRecommendationChangeset {
            priority_class: self.recommendation.priority().class,
            confidence: self.confidence as i16,
            lhr_set_hash: self.lhr_set_hash,
        }
    }
}

#[derive(AsChangeset)]
#[diesel(table_name = crate::schema::prefix_tree)]
struct PrefixRecommendationChangeset {
    pub priority_class: PriorityClass,
    pub confidence: i16,
    pub lhr_set_hash: LhrSetHash,
}

impl From<&SplitRecommendation> for SplitAnalysisResult {
    fn from(it: &SplitRecommendation) -> Self {
        let ReProbePriority {
            class,
            supporting_observations,
        } = *it.priority();
        let should_split = match it {
            SplitRecommendation::YesSplit { priority: _ } => Some(true),
            SplitRecommendation::NoKeep { priority: _ } => Some(false),
            SplitRecommendation::CannotDetermine { priority: _ } => None,
        };
        SplitAnalysisResult {
            class,
            evidence: supporting_observations,
            should_split,
            algo_version: recommend::ALGO_VERSION,
        }
    }
}

pub fn perform_prefix_split(
    conn: &mut PgConnection,
    context: context::Context,
    subnets: Subnets,
    blocklist: &PrefixBlocklist,
) -> Result<usize> {
    conn.transaction(|conn| {
        insert_split_subnets(conn, subnets, blocklist)?;
        mark_parent_obsolete(conn, context.node())
    })
    .context("in tx to perform prefix split")
}

fn insert_split_subnets(
    conn: &mut PgConnection,
    subnets: Subnets,
    blocklist: &PrefixBlocklist,
) -> Result<usize> {
    use crate::schema::prefix_tree::dsl::*;

    let base_merge = MergeStatus::new(subnets[0].subnet.network.prefix_len());
    let to_tuple = |it: &Subnet| {
        let merge = if blocklist.is_whole_net_blocked(&it.subnet.network) {
            MergeStatus::Blocked
        } else {
            base_merge
        };
        (
            net.eq6(&it.subnet.network),
            merge_status.eq(merge),
            lhr_set_hash.eq(it.lhr_set_hash()),
        )
    };
    let tuples = subnets.iter().map(to_tuple).collect_vec();

    let on_conflict = (
        // if already exists, it shouldn't be blocked, and if it is, we'd realise with the next split attempt
        merge_status.eq(base_merge),
        priority_class.eq(PriorityClass::HighFresh),
        // we can't set hash here (it's different for each subnet), but it will be computed with the next analysis
    );

    diesel::insert_into(prefix_tree)
        .values(tuples)
        .on_conflict(net)
        .do_update()
        .set(on_conflict)
        .execute(conn)
        .fix_cause()
        .context("inserting new split prefixes")
}

fn mark_parent_obsolete(conn: &mut PgConnection, parent: &PrefixTree) -> Result<usize> {
    use crate::schema::prefix_tree::dsl::*;

    diesel::update(parent)
        .set(merge_status.eq(parent.merge_status.split()))
        .execute(conn)
        .fix_cause()
        .context("updating parent with new merge status")
}

pub fn mark_as_blocked(conn: &mut PgConnection, context: &context::Context) -> Result<usize> {
    use crate::schema::prefix_tree::dsl::*;

    diesel::update(context.node())
        .set(merge_status.eq(MergeStatus::Blocked))
        .execute(conn)
        .fix_cause()
        .context("marking parent node as blocked")
}
