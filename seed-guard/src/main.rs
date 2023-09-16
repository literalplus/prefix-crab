use anyhow::Result;
use clap::Parser;

use prefix_crab::helpers::{bootstrap, logging};

use futures::executor;
use prefix_crab::helpers::stop::{self, flatten};
use tokio::try_join;

// FQNs are needed in some Diesel macros, make them easier to read
pub use db_model::persist;
pub use db_model::{schema, sql_types};

pub mod schedule;
pub mod as_list;
pub mod prefix_changeset;
pub mod tree_update;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[clap(flatten)]
    logging: logging::Params,

    #[clap(flatten)]
    persist: persist::Params,

    #[clap(flatten)]
    schedule: schedule::Params,

    #[clap(flatten)]
    tree_update: tree_update::Params,
}

fn main() -> Result<()> {
    bootstrap::run(Cli::parse, |cli: &Cli| &cli.logging, do_run)
}

fn do_run(cli: Cli) -> Result<()> {
    persist::initialize(&cli.persist)?;

    let sig_handler = stop::new();
    let stop_rx = sig_handler.subscribe_stop();
    tokio::spawn(sig_handler.wait_for_signal());

    let schedule_handle = tokio::spawn(schedule::run(
        stop_rx,
        cli.schedule,
    ));

    executor::block_on(async {
        try_join!(
            flatten(schedule_handle),
        )?;
        Ok(())
    })
}
