use queue_models::echo_response::EchoProbeResponse;
use anyhow::Result;
use log::{info, trace};
use tokio::sync::mpsc::Receiver;

#[derive(Debug)]
pub struct TaskRequest {
    pub model: EchoProbeResponse,
}

pub async fn run(mut task_rx: Receiver<TaskRequest>) -> Result<()> {
    info!("probe handler up & running!");
    loop {
        if let Some(batch) = task_rx.recv().await {
            trace!("Received something: {:?}", batch);
        } else {
            info!("probe handler shutting down.");
            return Ok(());
        }
    }
}
