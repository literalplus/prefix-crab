use anyhow::Result;
use clap::Parser;

use futures::executor;
use prefix_crab::helpers::stop::flatten;
use prefix_crab::helpers::{bootstrap, logging, stop};

/// Stores probe results in memory for the duration of the scan.
mod probe_store;
/// Handles batching of probe requests into yarrp calls.
mod schedule;
mod yarrp_call;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[clap(flatten)]
    logging: logging::Params,

    #[clap(flatten)]
    scheduler: schedule::Params,
}

fn main() -> Result<()> {
    bootstrap::run(Cli::parse, |cli: &Cli| &cli.logging, do_run)
}

fn do_run(cli: Cli) -> Result<()> {
    let scheduler_handle = tokio::spawn(schedule::run(cli.scheduler));

    let sig_handler = stop::new();
    let _stop_rx = sig_handler.subscribe_stop();
    tokio::spawn(sig_handler.wait_for_signal());

    executor::block_on(flatten(scheduler_handle))
}
