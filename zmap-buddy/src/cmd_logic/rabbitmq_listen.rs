use anyhow::{Context, Result};
use clap::Args;
use futures::executor;
use tokio::select;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::{rabbit, schedule};

#[derive(Args)]
pub struct Params {
    #[clap(flatten)]
    scheduler: schedule::Params,

    #[clap(flatten)]
    rabbit: rabbit::Params,
}

pub fn handle(params: Params) -> Result<()> {
    // TODO tune buffer size parameter
    // bounded s.t. we don't keep consuming new work items when the scheduler is blocked for some reason.
    let (task_tx, task_rx) = mpsc::channel(4096);
    let (res_tx, res_rx) = mpsc::unbounded_channel();

    // This task if shut down by the RabbitMQ receiver closing the channel
    let scheduler_handle = tokio::spawn(schedule::run(
        task_rx, res_tx, params.scheduler,
    ));

    let sig_handler = signal_handler::new();
    let stop_rx = sig_handler.subscribe_stop();
    tokio::spawn(sig_handler.wait_for_signal());

    let rabbit_handle = tokio::spawn(rabbit::run(
        task_tx, res_rx, stop_rx, params.rabbit,
    ));

    executor::block_on(wait_for_exit(scheduler_handle, rabbit_handle))
}

async fn wait_for_exit(
    scheduler_handle: JoinHandle<Result<()>>, rabbit_handle: JoinHandle<Result<()>>,
) -> Result<()> {
    let inner_res = select! {
        res = scheduler_handle => res.with_context(|| "failed to join scheduler"),
        res = rabbit_handle => res.with_context(|| "failed to join rabbit"),
    }?;
    inner_res.with_context(|| "a task exited unexpectedly")
}

mod signal_handler {
    use log::{info, warn};
    use tokio::select;
    use tokio::signal::unix::{signal, SignalKind};
    use tokio::sync::broadcast;

    pub struct SignalHandler {
        stop_tx: broadcast::Sender<()>,
    }

    pub fn new() -> SignalHandler {
        let (stop_tx, _) = broadcast::channel(1);
        SignalHandler { stop_tx }
    }

    impl SignalHandler {
        pub fn subscribe_stop(&self) -> broadcast::Receiver<()> {
            self.stop_tx.subscribe()
        }

        pub async fn wait_for_signal(self) {
            let mut sigterm = signal(SignalKind::terminate()).unwrap();
            let mut sigint = signal(SignalKind::interrupt()).unwrap();
            loop {
                select! {
                _ = sigterm.recv() => info!("Terminated; stopping..."),
                _ = sigint.recv() => info!("Interrupted; stopping..."),
                }
                if let Err(e) = self.stop_tx.send(()) {
                    warn!("Failed to notify tasks to stop, maybe they're already finished. {}", e);
                }
                break;
            }
        }
    }
}
