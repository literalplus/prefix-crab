use log::info;
use queue_models::probe_request::{ProbeRequest, TraceRequest};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use anyhow::*;

use crate::analyse::EchoFollowUp;

pub async fn run(
    probe_tx: UnboundedSender<ProbeRequest>,
    mut follow_up_rx: UnboundedReceiver<EchoFollowUp>,
) -> Result<()> {
    info!("Scheduler is ready for work.");
    loop {
        if let Some(req) = follow_up_rx.recv().await {
            probe_tx.send(req.into())?;
        } else {
            info!("Scheduler shutting down.");
            return Ok(());
        }
    }
}

impl From<EchoFollowUp> for ProbeRequest {
    fn from(value: EchoFollowUp) -> Self {
        let msg = TraceRequest {
            id: value.id,
            targets: value.targets,
            were_responsive: value.for_responsive,
        };
        ProbeRequest::Trace(msg)
    }
}