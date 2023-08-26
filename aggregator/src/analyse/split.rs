use anyhow::{Context, Result};
use diesel::{prelude::*, PgConnection, QueryDsl, SelectableHelper};
use log::debug;

use crate::{
    persist::dsl::CidrMethods,
    prefix_tree::ContextOps,
    schema::measurement_tree::{dsl::measurement_tree, target_net},
    persist::DieselErrorFixCause,
};

use self::subnet::Subnets;

use super::{context, MeasurementTree};

mod subnet;
mod recommend;

pub fn process(conn: &mut PgConnection, request: &context::Context) -> Result<()> {
    let relevant_measurements = measurement_tree
        .filter(target_net.subnet_or_eq(request.node().path))
        .select(MeasurementTree::as_select())
        .load(conn)
        .fix_cause()
        .with_context(|| {
            format!(
                "Unable to load existing measurements for {}",
                request.log_id()
            )
        })?;
    let base_net = request.node().try_net_into_v6()?;
    let subnets = Subnets::new(base_net, relevant_measurements)?;
    let rec = recommend::recommend(subnets);
    debug!("For {}, the department is: Parks & {:?}", request.log_id(), rec);
    Ok(())
}

