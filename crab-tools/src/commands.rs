use anyhow::Result;
use clap::Subcommand;
use log::debug;

mod prefix_inspect;
mod prefix_scan;
mod rate_calculate;
mod test_metrics;

pub fn handle(cmd: Commands) -> Result<()> {
    let command_result = match cmd {
        Commands::PrefixScan(data) => prefix_scan::handle(data),
        Commands::PrefixInspect(data) => prefix_inspect::handle(data),
        Commands::RateCalculate(data) => rate_calculate::handle(data),
        Commands::TestMetrics(data) => test_metrics::handle(data),
    };
    debug!("Finished command execution. Result: {:?}", command_result);
    command_result
}

#[derive(Subcommand)]
pub enum Commands {
    PrefixScan(prefix_scan::Params),
    PrefixInspect(prefix_inspect::Params),
    RateCalculate(rate_calculate::Params),
    TestMetrics(test_metrics::Params),
}
