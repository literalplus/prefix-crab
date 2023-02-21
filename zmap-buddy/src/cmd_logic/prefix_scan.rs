use anyhow::Result;
use clap::Args;
use ipnet::Ipv6Net;

#[derive(Args)]
pub struct Params {
    #[clap(flatten)]
    base: super::ZmapBaseParams,

    target_prefix: Ipv6Net,
}

pub fn handle(params: Params) -> Result<()> {
    params.base.into_caller()?;
    panic!("not implemented :(");
}
