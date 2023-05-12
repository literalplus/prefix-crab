use anyhow::Result;
use clap::Args;
use ipnet::Ipv6Net;
use log::trace;
use crate::prefix_split;

use crate::zmap_call::{self, TargetCollector};

#[derive(Args)]
pub struct Params {
    #[clap(flatten)]
    base: zmap_call::Params,

    target_prefix: Ipv6Net,
}

pub fn handle(params: Params) -> Result<()> {
    let caller = params.base.to_caller_verifying_sudo()?;
    let splits = prefix_split::process(params.target_prefix)?;
    trace!("Subnet splits: {:?}", splits);
    let mut targets = TargetCollector::new_default()?;
    for split in splits {
        // TODO permute these to spread load a bit
        for address in split.addresses {
            targets.push(address)?;
        }
    }
    caller.consume_run(targets)
}
