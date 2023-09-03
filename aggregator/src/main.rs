use anyhow::{Context, Result};
use clap::Parser;

use prefix_crab::helpers::{bootstrap, logging};

use futures::executor;
use prefix_crab::helpers::signal_handler;
use tokio::select;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

// FQNs are needed in some Diesel macros, make them easier to read
pub use persist::schema::{self, sql_types};

/// Analysis of incoming data in combination with existing knowledge
mod analyse;
/// Business logic for handling incoming probes
mod handle_probe;
/// Persistence-specific conversions & DSL
mod persist;
/// Keeping track of prefix information in a tree structure
mod prefix_tree;
/// RabbitMQ-specific logic (producers, consumers), encapsulated using in-memory senders/receivers
mod rabbit;
/// Scheduling new analyses based on priority and capacity
mod schedule;
#[cfg(test)]
mod test_utils;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[clap(flatten)]
    logging: logging::Params,

    #[clap(flatten)]
    rabbit: rabbit::Params,

    #[clap(flatten)]
    persist: persist::Params,
}

fn main() -> Result<()> {
    bootstrap::run(Cli::parse, |cli: &Cli| &cli.logging, do_run)
}

fn do_run(cli: Cli) -> Result<()> {
    // TODO tune buffer size parameter
    // bounded s.t. we don't keep consuming new work items when we block for some reason
    let (task_tx, task_rx) = mpsc::channel(4096);
    let (ack_tx, ack_rx) = mpsc::unbounded_channel();
    let (probe_tx, probe_rx) = mpsc::unbounded_channel();
    let (follow_up_tx, follow_up_rx) = mpsc::unbounded_channel();

    persist::initialize(&cli.persist)?;

    // This task is shut down by the RabbitMQ receiver closing the channel
    let probe_handle = tokio::spawn(handle_probe::run(task_rx, ack_tx, follow_up_tx));
    let schedule_handle = tokio::spawn(schedule::run(probe_tx, follow_up_rx));

    let sig_handler = signal_handler::new();
    let stop_rx = sig_handler.subscribe_stop();
    tokio::spawn(sig_handler.wait_for_signal());

    let rabbit_handle = tokio::spawn(rabbit::run(task_tx, ack_rx, probe_rx, stop_rx, cli.rabbit));

    executor::block_on(wait_for_exit(probe_handle, rabbit_handle, schedule_handle))
}

async fn wait_for_exit(
    probe_handle: JoinHandle<Result<()>>,
    rabbit_handle: JoinHandle<Result<()>>,
    schedule_handle: JoinHandle<Result<()>>,
) -> Result<()> {
    let inner_res = select! {
        res = probe_handle => res.with_context(|| "failed to join probe handler"),
        res = rabbit_handle => res.with_context(|| "failed to join rabbit"),
        res = schedule_handle => res.with_context(|| "failed to join scheduler"),
    }?;
    inner_res.with_context(|| "a task exited unexpectedly")
}
