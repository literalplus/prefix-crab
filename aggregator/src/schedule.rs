use anyhow::*;
use log::info;
use queue_models::probe_request::{ProbeRequest, TraceRequest, TraceRequestId};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{analyse::EchoFollowUp, prefix_tree::PrefixTree};

#[derive(Debug)]
pub struct FollowUpRequest {
    pub id: TraceRequestId,
    pub prefix_tree: PrefixTree,
    pub follow_ups: Vec<EchoFollowUp>,
}

pub async fn run(
    probe_tx: UnboundedSender<ProbeRequest>,
    mut follow_up_rx: UnboundedReceiver<FollowUpRequest>,
) -> Result<()> {
    info!("Scheduler is ready for work.");
    loop {
        if let Some(req) = follow_up_rx.recv().await {
            probe_tx.send(flatten_request(req))?;
        } else {
            info!("Scheduler shutting down.");
            return Ok(());
        }
    }
}

fn flatten_request(req: FollowUpRequest) -> ProbeRequest {
    // TODO skip less-useful requests with a probability (e.g. if prefix is already 100% analysed;
    // but then we also need to clear the DB flag that we are waiting for a response)

    // TODO reduce number of targets if many are requested, we already know enough, or sth like that
    let targets = req
        .follow_ups
        .into_iter()
        .flat_map(|it| it.targets)
        .collect();

    let msg = TraceRequest {
        id: req.id,
        targets,
    };
    ProbeRequest::Trace(msg)
}
