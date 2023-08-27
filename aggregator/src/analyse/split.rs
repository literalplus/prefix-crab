use anyhow::{Context, Result};
use diesel::{prelude::*, PgConnection, QueryDsl, SelectableHelper};
use ipnet::Ipv6Net;
use log::debug;

use crate::{
    persist::dsl::CidrMethods,
    persist::DieselErrorFixCause, prefix_tree::ContextOps,
};

use self::subnet::Subnets;

use super::{context, MeasurementTree, SplitAnalysisResult};

pub use confidence::{Confidence, MAX_CONFIDENCE};

mod confidence;
mod persist;
mod recommend;
mod subnet;

pub fn process(conn: &mut PgConnection, request: context::Context) -> Result<()> {
    let relevant_measurements = load_relevant_measurements(conn, &request.node().net)?;
    let subnets = Subnets::new(request.node().net, relevant_measurements)?;
    let rec = recommend::recommend(&subnets);
    debug!(
        "For {}, the department is: Parks & {:?}",
        request.log_id(),
        rec
    );
    let confidence = confidence::rate(&request, &rec);
    persist::save_recommendation(conn, &request, &rec, confidence)?;
    if confidence >= MAX_CONFIDENCE {
        persist::perform_prefix_split(conn, request, subnets)?;
    }
    Ok(())
}

fn load_relevant_measurements(
    conn: &mut PgConnection,
    base_net: &Ipv6Net,
) -> Result<Vec<MeasurementTree>> {
    use crate::schema::measurement_tree::dsl::*;

    measurement_tree
        .filter(target_net.subnet_or_eq6(base_net))
        .select(MeasurementTree::as_select())
        .load(conn)
        .fix_cause()
        .with_context(|| format!("loading relevant measurements for {}", base_net))
}
