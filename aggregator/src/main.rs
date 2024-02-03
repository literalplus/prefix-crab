use anyhow::Result;
use clap::Parser;

use prefix_crab::helpers::{bootstrap, logging};

use futures::executor;
use prefix_crab::helpers::stop::{self, flatten};
use tokio::sync::mpsc;
use tokio::try_join;

// FQNs are needed in some Diesel macros, make them easier to read
pub use db_model::persist;
pub use db_model::{schema, sql_types};

/// Analysis of incoming data in combination with existing knowledge
mod analyse;
/// Business logic for handling incoming probes
mod handle_probe;
/// RabbitMQ-specific logic (producers, consumers), encapsulated using in-memory senders/receivers
mod rabbit;
/// Scheduling new analyses based on priority and capacity
mod schedule;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[clap(flatten)]
    logging: logging::Params,

    #[clap(flatten)]
    rabbit: rabbit::Params,

    #[clap(flatten)]
    persist: persist::Params,

    #[clap(flatten)]
    schedule: schedule::Params,

    #[clap(flatten)]
    handle_probe: handle_probe::Params,
}

fn main() -> Result<()> {
    bootstrap::run(Cli::parse, |cli: &Cli| &cli.logging, do_run)
}

fn do_run(cli: Cli) -> Result<()> {
    // TODO tune buffer size parameter
    // bounded s.t. we don't keep consuming new work items when we block for some reason
    let (result_tx, result_rx) = mpsc::channel(2);
    let (ack_tx, ack_rx) = mpsc::channel(32);
    let (probe_tx, probe_rx) = mpsc::channel(64);
    let (follow_up_tx, follow_up_rx) = mpsc::channel(64);

    persist::initialize(&cli.persist)?;

    // This task is shut down by the RabbitMQ receiver closing the channel
    let probe_handle = tokio::spawn(handle_probe::run(
        result_rx,
        ack_tx,
        follow_up_tx,
        cli.handle_probe,
    ));

    let sig_handler = stop::new();
    let stop_rx = sig_handler.subscribe_stop();
    tokio::spawn(sig_handler.wait_for_signal());

    let schedule_handle = tokio::spawn(schedule::run(
        probe_tx,
        follow_up_rx,
        stop_rx.clone(),
        cli.schedule,
    ));

    let rabbit_handle = tokio::spawn(rabbit::run(result_tx, ack_rx, probe_rx, stop_rx, cli.rabbit));

    executor::block_on(async {
        try_join!(
            flatten(schedule_handle),
            flatten(rabbit_handle),
            flatten(probe_handle)
        )?;
        Ok(())
    })
}
