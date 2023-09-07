use log::info;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio_util::sync::CancellationToken;

pub struct SignalHandler {
    tok: CancellationToken,
}

pub fn new() -> SignalHandler {
    SignalHandler { tok: CancellationToken::new() }
}

impl SignalHandler {
    pub fn subscribe_stop(&self) -> CancellationToken {
        self.tok.clone()
    }

    pub async fn wait_for_signal(self) {
        let mut sigterm = signal(SignalKind::terminate()).unwrap();
        let mut sigint = signal(SignalKind::interrupt()).unwrap();
        let mut sighup = signal(SignalKind::hangup()).unwrap();

        select! {
            _ = sigterm.recv() => info!("Terminated; stopping..."),
            _ = sigint.recv() => info!("Interrupted; stopping..."),
            _ = sighup.recv() => info!("Hangup received; stopping..."), // used by tmux apparently
            }
        self.tok.cancel();
    }
}
