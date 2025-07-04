use diesel::{prelude::*, PgConnection, QueryDsl, SelectableHelper};
use ipnet::Ipv6Net;
use log::{debug, info, warn};
use prefix_crab::blocklist::PrefixBlocklist;
use thiserror::Error;
use tracing::instrument;

use crate::{observe, persist::{dsl::CidrMethods, DieselErrorFixCause}};
use db_model::prefix_tree::ContextOps;

use self::subnet::Subnets;

use super::{context, MeasurementTree, SplitAnalysisResult};

pub use db_model::analyse::{Confidence, CONFIDENCE_THRESH};

mod collapse;
mod confidence;
mod merge_redundant;
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

#[instrument(skip_all, fields(net = %request.parent.node.net))]
pub fn process(
    conn: &mut PgConnection,
    request: context::Context,
    blocklist: &PrefixBlocklist,
) -> SplitResult<()> {
    if !request.node().merge_status.is_eligible_for_split() {
        tracing::warn!("no longer a leaf");
        warn!(
            "Handled prefix is (no longer?) a leaf, split not possible: {:?}",
            request.node().net,
        );
        return Ok(());
    } else if blocklist.is_whole_net_blocked(&request.node().net) {
        tracing::warn!("blocked");
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
    let hash = subnets.combined_lhr_set_hash();

    persist::save_recommendation(conn, &request, &rec, confidence, hash)
        .map_err(|source| SplitError::SaveRecommendation { source })?;

    if confidence >= CONFIDENCE_THRESH {
        if rec.should_split() {
            info!(
                "Splitting prefix {} due to recommendation {:?} at {}% confidence.",
                request.log_id(),
                rec,
                confidence
            );
            observe::record_split_decision(rec.priority().class, true, rec.should_split());
            persist::perform_prefix_split(conn, request, subnets, blocklist)
                .map_err(|source| SplitError::PerformSplit { source })?;
        } else {
            if confidence < Confidence::MAX {
                debug!(
                    "Keeping prefix {} due to recommendation {:?} at {}% confidence.",
                    request.log_id(),
                    rec,
                    confidence
                );
                observe::record_split_decision(rec.priority().class, false, rec.should_split());
            } else {
                // If we reach 255% of the threshold without splitting (any doubt would immediately split at this point),
                // optimise the measurement trees down to 16 (instead of one per /64 -> huge savings for bigger prefixes)
                info!(
                    "Keeping prefix {} due to recommendation {:?} at max {}% confidence. Collapsing measurements.",
                    request.log_id(),
                    rec,
                    confidence
                );
                observe::record_split_decision(rec.priority().class, true, rec.should_split());
                collapse::process(conn, &request, relevant_measurements)
                    .map_err(|source| SplitError::CollapseMeasurements { source })?;
            }

            if confidence > 200 {
                if let Err(e) = merge_redundant::process(conn, &request) {
                    warn!(
                        "Failed to process redundancy check for {} - {:?}",
                        request.log_id(),
                        e
                    )
                }
            }
        }
    } else {
        debug!(
            "No action on prefix {} due to recommendation {:?} at insufficient {}% confidence.",
            request.log_id(),
            rec,
            confidence
        );
        observe::record_split_decision(rec.priority().class, false, rec.should_split());
    }
    Ok(())
}

#[instrument(skip(conn))]
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
