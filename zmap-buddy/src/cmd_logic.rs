use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use log::debug;

use crate::zmap_call::Caller;

pub mod one_shot;
pub mod prefix_scan;

pub fn handle(cmd: Commands) -> Result<()> {
    let command_result = match cmd {
        Commands::OneShot(data) => one_shot::handle(data),
        Commands::PrefixScan(data) => prefix_scan::handle(data),
    };
    debug!("Finished command execution. Result: {:?}", command_result);
    command_result
}

#[derive(Subcommand)]
pub enum Commands {
    /// Perform a single call to zmap, probing given target addresses.
    OneShot(one_shot::Params),

    /// Scan one level of prefix for responsive sub-prefixes.
    PrefixScan(prefix_scan::Params),
}


#[derive(Args)]
pub struct ZmapBaseParams {
    #[arg(long)]
    source_address: String,

    /// FQ path to zmap binary
    #[arg(long, default_value = "/usr/local/sbin/zmap")]
    bin_path: String,

    /// FQ path to sudo binary
    #[arg(long, default_value = "/usr/bin/sudo")]
    sudo_path: String,
}

impl ZmapBaseParams {
    fn into_caller(self) -> Result<Caller> {
        let mut caller = Caller::new(self.sudo_path, self.bin_path);
        debug!("Using zmap caller: {:?}", caller);
        caller.verify_sudo_access()
            .with_context(|| "If not using NOPASSWD, you might need to re-run sudo manually.")?;
        caller.push_source_address(self.source_address)?;
        return Ok(caller);
    }
}
