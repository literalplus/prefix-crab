use anyhow::*;
use clap::Args;
use db_model::{persist, prefix_tree::PrefixTree};
use futures::executor;
use ipnet::Ipv6Net;
use log::info;


#[derive(Args, Clone)]
pub struct Params {
    #[clap(flatten)]
    persist: persist::Params,

    target_prefix: Ipv6Net,
}

pub fn handle(params: Params) -> Result<()> {



    Ok(())
}
