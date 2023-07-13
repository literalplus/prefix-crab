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

        select! {
            _ = sigterm.recv() => info!("Terminated; stopping..."),
            _ = sigint.recv() => info!("Interrupted; stopping..."),
            }
        if let Err(e) = self.stop_tx.send(()) {
            warn!("Failed to notify tasks to stop, maybe they're already finished. {}", e);
        }
    }
}
