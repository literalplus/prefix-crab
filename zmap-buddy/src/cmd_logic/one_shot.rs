use std::net::Ipv6Addr;

use anyhow::Result;
use clap::Args;

use crate::zmap_call::{self, TargetCollector};

#[derive(Args)]
pub struct Params {
    #[clap(flatten)]
    base: zmap_call::Params,

    target_addresses: Vec<Ipv6Addr>,
}

pub fn handle(params: Params) -> Result<()> {
    let caller = params.base.to_caller_verifying_sudo()?;
    let targets = if params.target_addresses.is_empty() {
        [
            "fdf9:d3a4:2fff:96ec::a", "fd00:aff1:3::a", "fd00:aff1:3::3a",
            "fd00:aff1:3::c", "fd00:aff1:678::b", "2a02:8388:8280:ec80:3a43:7dff:febe:998",
            "2a02:8388:8280:ec80:3a43:7dff:febe:999"
        ].iter()
            .map(|it| it.to_string().parse::<Ipv6Addr>().expect("valid hard-coded IPv6"))
            .collect()
    } else {
        params.target_addresses
    };

    let collector = TargetCollector::from_vec(targets)?;
    caller.consume_run(collector)
}
