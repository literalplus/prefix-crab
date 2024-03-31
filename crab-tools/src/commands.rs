use anyhow::Result;
use clap::Subcommand;
use log::debug;

mod edge_analyse;
mod hit_count;
mod prefix_inspect;
mod prefix_scan;
mod rate_calculate;
mod test_metrics;
mod tree_compare;
mod uniform_merge;

pub fn handle(cmd: Commands) -> Result<()> {
    let command_result = match cmd {
        Commands::PrefixScan(data) => prefix_scan::handle(data),
        Commands::PrefixInspect(data) => prefix_inspect::handle(data),
        Commands::RateCalculate(data) => rate_calculate::handle(data),
        Commands::TestMetrics(data) => test_metrics::handle(data),
        Commands::EdgeAnalyse(data) => edge_analyse::handle(data),
        Commands::HitCount(data) => hit_count::handle(data),
        Commands::TreeCompare(data) => tree_compare::handle(data),
        Commands::UniformMerge(data) => uniform_merge::handle(data),
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
    EdgeAnalyse(edge_analyse::Params),   // evaluation E
    HitCount(hit_count::Params),         // evaluation A
    TreeCompare(tree_compare::Params),   // evaluation F
    UniformMerge(uniform_merge::Params), // evaluation G
}
