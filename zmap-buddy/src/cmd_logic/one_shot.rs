use anyhow::{Context, Result};
use clap::Args;

use crate::zmap_call::TargetCollector;

#[derive(Args)]
pub struct Params {
    #[clap(flatten)]
    base: super::ZmapBaseParams,

    target_addresses: Vec<String>,
}

pub fn handle(params: Params) -> Result<()> {
    let mut caller = params.base.into_caller()?;
    let targets = if params.target_addresses.is_empty() {
        [
            "fdf9:d3a4:2fff:96ec::a", "fd00:aff1:3::a", "fd00:aff1:3::3a",
            "fd00:aff1:3::c", "fd00:aff1:678::b", "2a02:8388:8280:ec80:3a43:7dff:febe:998",
            "2a02:8388:8280:ec80:3a43:7dff:febe:999"
        ].iter().map(|static_str| static_str.to_string()).collect()
    } else {
        params.target_addresses
    };

    let mut collector = TargetCollector::from_vec(targets)?;
    caller.consume_run(collector)
}
