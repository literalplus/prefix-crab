use anyhow::{Result};
use clap::Parser;

use prefix_crab::helpers::{bootstrap, logging};

mod cmd_logic;
mod zmap_call;
/// Handles splitting of prefixes & selection of addresses to scan in them.
mod prefix_split;
/// Stores probe results in memory for the duration of the scan.
mod probe_store;
/// Handles reception, sending, & translation of messages from/to RabbitMQ.
mod rabbit;
/// Handles batching of probe requests into ZMAP calls.
mod schedule;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[clap(flatten)]
    logging: logging::Params,

    #[command(subcommand)]
    command: cmd_logic::Commands,
}

fn main() -> Result<()> {
    bootstrap::run(
        Cli::parse,
        |cli: &Cli| &cli.logging,
        do_run,
    )
}

fn do_run(cli: Cli) -> Result<()> {
    cmd_logic::handle(cli.command)
}
