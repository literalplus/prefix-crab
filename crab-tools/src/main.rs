use anyhow::Result;
use clap::Parser;

use prefix_crab::helpers::{bootstrap, logging};

mod commands;
mod rabbit;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[clap(flatten)]
    logging: logging::Params,

    #[command(subcommand)]
    command: commands::Commands,
}


fn main() -> Result<()> {
    bootstrap::run(
        Cli::parse,
        |cli: &Cli| &cli.logging,
        do_run,
    )
}

fn do_run(cli: Cli) -> Result<()> {
    commands::handle(cli.command)
}
