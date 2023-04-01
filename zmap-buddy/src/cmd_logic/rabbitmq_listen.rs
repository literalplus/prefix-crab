use anyhow::Result;
use clap::Args;
use futures::executor;
use tokio::select;
use tokio::sync::mpsc;

mod rabbit_receiver;
mod zmap_queue;

#[derive(Args)]
pub struct Params {
    #[clap(flatten)]
    base: super::ZmapBaseParams,

    #[clap(flatten)]
    rabbit: rabbit_receiver::RabbitParams,
}

pub fn handle(params: Params) -> Result<()> {
    let (task_tx, task_rx) = mpsc::channel(4096);
    // This task if shut down by the RabbitMQ receiver closing the channel
    let queue_handle = tokio::spawn(zmap_queue::start_handler(
        task_rx, params.base.clone(),
    ));

    let sig_handler = signal_handler::new();
    let stop_rx = sig_handler.subscribe_stop();
    tokio::spawn(sig_handler.wait_for_signal());

    let rabbit_handle = tokio::spawn(rabbit_receiver::start_receiver(
        task_tx, stop_rx, params.rabbit,
    ));

    executor::block_on(async move {
        select! {
            Err(e) = queue_handle => Err(e),
            res = rabbit_handle => res,
        }
    })?
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
