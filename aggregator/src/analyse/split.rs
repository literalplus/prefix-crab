use diesel::{prelude::*, PgConnection, QueryDsl, SelectableHelper};
use ipnet::Ipv6Net;
use log::{debug, info, warn};
use thiserror::Error;

use crate::{
    persist::dsl::CidrMethods,
    persist::DieselErrorFixCause,
};
use db_model::prefix_tree::{ContextOps, MergeStatus};

use self::subnet::Subnets;

use super::{context, MeasurementTree, SplitAnalysisResult};

pub use db_model::analyse::{Confidence, MAX_CONFIDENCE};

mod confidence;
mod persist;
mod recommend;
mod subnet;

#[derive(Error, Debug)]
pub enum SplitError {
    #[error("database error loading relevant measurements fpr {base_net} from the database")]
    LoadMeasurementsFromDb { source: anyhow::Error, base_net: Ipv6Net },
    #[error("unable to split prefix into subnets")]
    SplitSubnets { source: anyhow::Error },
    #[error("unable to save recommendation to database")]
    SaveRecommendation { source: anyhow::Error },
    #[error("unable to perform prefix split")]
    PerformSplit { source: anyhow::Error },
}

pub type SplitResult<T> = std::result::Result<T, SplitError>;

pub fn process(conn: &mut PgConnection, request: context::Context) -> SplitResult<()> {
    if request.node().merge_status != MergeStatus::Leaf {
        warn!(
            "Handled prefix is (no longer?) a leaf, split not possible: {:?}",
            request.node().net,
        );
        return Ok(());
    }
    let relevant_measurements = load_relevant_measurements(conn, &request.node().net)?;
    let subnets = Subnets::new(request.node().net, relevant_measurements)
        .map_err(|source| SplitError::SplitSubnets { source })?;
    let rec = recommend::recommend(&subnets);
    debug!(
        "For {}, the department is: Parks & {:?}",
        request.log_id(),
        rec
    );
    let confidence = confidence::rate(request.node().net, &rec);
    persist::save_recommendation(conn, &request, &rec, confidence)
        .map_err(|source| SplitError::SaveRecommendation { source })?;
    if confidence >= MAX_CONFIDENCE {
        if rec.should_split() {
            info!(
                "Splitting prefix {} due to recommendation {:?} at {}% confidence.",
                request.log_id(),
                rec.priority().class,
                confidence
            );
            persist::perform_prefix_split(conn, request, subnets)
                .map_err(|source| SplitError::PerformSplit { source })?;
        } else {
            debug!(
                "Keeping prefix {} due to recommendation {:?} at {}% confidence.",
                request.log_id(),
                rec.priority().class,
                confidence
            );
        }
    } else {
        debug!(
            "No action on prefix {} due to recommendation {:?} at insufficient {}% confidence.",
            request.log_id(),
            rec.priority().class,
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
        .map_err(|source| SplitError::LoadMeasurementsFromDb { source, base_net: *base_net })
}
