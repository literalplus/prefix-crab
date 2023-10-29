use anyhow::Result;
use clap::Subcommand;
use log::debug;

mod prefix_scan;

pub fn handle(cmd: Commands) -> Result<()> {
    let command_result = match cmd {
        Commands::PrefixScan(data) => prefix_scan::handle(data),
    };
    debug!("Finished command execution. Result: {:?}", command_result);
    command_result
}

#[derive(Subcommand)]
pub enum Commands {
    /// Scan one level of prefix for responsive sub-prefixes via zmap-buddy.
    PrefixScan(prefix_scan::Params),
}
