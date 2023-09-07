use anyhow::{Result, anyhow};
use clap::Parser;

use futures::executor;
use prefix_crab::helpers::{bootstrap, logging, signal_handler};
use tokio::try_join;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Stores probe results in memory for the duration of the scan.
mod probe_store;
/// Handles reception, sending, & translation of messages from/to RabbitMQ.
mod rabbit;
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

    #[clap(flatten)]
    rabbit: rabbit::Params,
}

fn main() -> Result<()> {
    bootstrap::run(Cli::parse, |cli: &Cli| &cli.logging, do_run)
}

fn do_run(cli: Cli) -> Result<()> {
    // TODO tune buffer size parameter
    // bounded s.t. we don't keep consuming new work items when the scheduler is blocked for some reason.
    let (task_tx, task_rx) = mpsc::channel(4096);
    let (res_tx, res_rx) = mpsc::unbounded_channel();

    // This task if shut down by the RabbitMQ receiver closing the channel
    let scheduler_handle = tokio::spawn(schedule::run(task_rx, res_tx, cli.scheduler));

    let sig_handler = signal_handler::new();
    let stop_rx = sig_handler.subscribe_stop();
    tokio::spawn(sig_handler.wait_for_signal());

    let rabbit_handle = tokio::spawn(rabbit::run(task_tx, res_rx, stop_rx, cli.rabbit));

    executor::block_on(async {
        try_join!(flatten(scheduler_handle), flatten(rabbit_handle))?;
        Ok(())
    })
}

async fn flatten(handle: JoinHandle<Result<()>>) -> Result<()> {
    match handle.await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(err)) => Err(err),
        Err(err) => Err(anyhow!(err)),
    }
}
