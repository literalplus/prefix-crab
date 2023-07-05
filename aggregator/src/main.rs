use anyhow::{Context, Result};
use clap::Parser;

use prefix_crab::helpers::{bootstrap, logging};

use futures::executor;
use tokio::select;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use prefix_crab::helpers::signal_handler;

mod rabbit;
mod models;
mod schema;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[clap(flatten)]
    logging: logging::Params,

    #[clap(flatten)]
    rabbit: rabbit::Params,

    #[clap(flatten)]
    handler: handle_probe::Params,
}

fn main() -> Result<()> {
    bootstrap::run(
        Cli::parse,
        |cli: &Cli| &cli.logging,
        do_run,
    )
}

fn do_run(cli: Cli) -> Result<()> {
    // TODO tune buffer size parameter
    // bounded s.t. we don't keep consuming new work items when the scheduler is blocked for some reason.
    let (task_tx, task_rx) = mpsc::channel(4096);

    // This task is shut down by the RabbitMQ receiver closing the channel
    let scheduler_handle = tokio::spawn(handle_probe::run(
        task_rx, cli.handler
    ));

    let sig_handler = signal_handler::new();
    let stop_rx = sig_handler.subscribe_stop();
    tokio::spawn(sig_handler.wait_for_signal());

    let rabbit_handle = tokio::spawn(rabbit::run(
        task_tx, stop_rx, cli.rabbit,
    ));

    executor::block_on(wait_for_exit(scheduler_handle, rabbit_handle))
}

async fn wait_for_exit(
    probe_handle: JoinHandle<Result<()>>, rabbit_handle: JoinHandle<Result<()>>,
) -> Result<()> {
    let inner_res = select! {
        res = probe_handle => res.with_context(|| "failed to join probe handler"),
        res = rabbit_handle => res.with_context(|| "failed to join rabbit"),
    }?;
    inner_res.with_context(|| "a task exited unexpectedly")
}

mod handle_probe;