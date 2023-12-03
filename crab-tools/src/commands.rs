use anyhow::Result;
use clap::Subcommand;
use log::debug;

mod prefix_scan;
mod prefix_inspect;

pub fn handle(cmd: Commands) -> Result<()> {
    let command_result = match cmd {
        Commands::PrefixScan(data) => prefix_scan::handle(data),
        Commands::PrefixInspect(data) => prefix_inspect::handle(data),
    };
    debug!("Finished command execution. Result: {:?}", command_result);
    command_result
}

#[derive(Subcommand)]
pub enum Commands {
    PrefixScan(prefix_scan::Params),
    PrefixInspect(prefix_inspect::Params),
}
