use diesel::{prelude::*, PgConnection, QueryDsl, SelectableHelper};
use ipnet::Ipv6Net;
use log::{debug, info, warn};
use prefix_crab::blocklist::PrefixBlocklist;
use thiserror::Error;

use crate::{persist::dsl::CidrMethods, persist::DieselErrorFixCause};
use db_model::prefix_tree::ContextOps;

use self::subnet::Subnets;

use super::{context, MeasurementTree, SplitAnalysisResult};

pub use db_model::analyse::{Confidence, CONFIDENCE_THRESH};

mod collapse;
mod confidence;
mod persist;
mod recommend;
mod subnet;

#[derive(Error, Debug)]
pub enum SplitError {
    #[error("database error loading relevant measurements fpr {base_net} from the database")]
    LoadMeasurementsFromDb {
        source: anyhow::Error,
        base_net: Ipv6Net,
    },
    #[error("unable to split prefix into subnets")]
    SplitSubnets { source: anyhow::Error },
    #[error("unable to save recommendation to database")]
    SaveRecommendation { source: anyhow::Error },
    #[error("unable to perform prefix split")]
    PerformSplit { source: anyhow::Error },
    #[error("unable to apply blocked state to node (blocklist)")]
    MarkBlocked { source: anyhow::Error },
    #[error("unable to collapse measurement tree at max confidence")]
    CollapseMeasurements { source: anyhow::Error },
}

pub type SplitResult<T> = std::result::Result<T, SplitError>;

pub fn process(
    conn: &mut PgConnection,
    request: context::Context,
    blocklist: &PrefixBlocklist,
) -> SplitResult<()> {
    if !request.node().merge_status.is_eligible_for_split() {
        warn!(
            "Handled prefix is (no longer?) a leaf, split not possible: {:?}",
            request.node().net,
        );
        return Ok(());
    } else if blocklist.is_whole_net_blocked(&request.node().net) {
        info!(
            "Entire prefix {} is blocked, marking the node as such.",
            request.node().net
        );
        return persist::mark_as_blocked(conn, &request)
            .map(|_| ())
            .map_err(|source| SplitError::MarkBlocked { source });
    }

    let relevant_measurements = load_relevant_measurements(conn, &request.node().net)?;
    let subnets = Subnets::new(request.node().net, &relevant_measurements)
        .map_err(|source| SplitError::SplitSubnets { source })?;
    let rec = recommend::recommend(&subnets);
    let confidence = confidence::rate(request.node().net, &rec);
    persist::save_recommendation(conn, &request, &rec, confidence)
        .map_err(|source| SplitError::SaveRecommendation { source })?;
    if confidence >= CONFIDENCE_THRESH {
        if rec.should_split() {
            info!(
                "Splitting prefix {} due to recommendation {:?} at {}% confidence.",
                request.log_id(),
                rec,
                confidence
            );
            persist::perform_prefix_split(conn, request, subnets, blocklist)
                .map_err(|source| SplitError::PerformSplit { source })?;
        } else if confidence < Confidence::MAX {
            debug!(
                "Keeping prefix {} due to recommendation {:?} at {}% confidence.",
                request.log_id(),
                rec,
                confidence
            );
        } else {
            // If we reach 255% of the threshold without splitting (any doubt would immediately split at this point),
            // optimise the measurement trees down to 16 (instead of one per /64 -> huge savings for bigger prefixes)
            // and mark the prefix as "final" to reduce measurement budget going toward it
            info!(
                "Keeping prefix {} due to recommendation {:?} at max {}% confidence. Collapsing measurements.",
                request.log_id(),
                rec,
                confidence
            );
            collapse::process(conn, request, relevant_measurements)
                .map_err(|source| SplitError::CollapseMeasurements { source })?;
        }
    } else {
        debug!(
            "No action on prefix {} due to recommendation {:?} at insufficient {}% confidence.",
            request.log_id(),
            rec,
            confidence
        );
    }
    Ok(())
}

fn load_relevant_measurements(
    conn: &mut PgConnection,
    base_net: &Ipv6Net,
) -> SplitResult<Vec<MeasurementTree>> {
    use crate::schema::measurement_tree::dsl::*;

    measurement_tree
        .filter(target_net.subnet_or_eq6(base_net))
        .select(MeasurementTree::as_select())
        .load(conn)
        .fix_cause()
        .map_err(|source| SplitError::LoadMeasurementsFromDb {
            source,
            base_net: *base_net,
        })
}
