use std::net::Ipv6Addr;

use anyhow::*;
use db_model::prefix_tree::PrefixTree;
use log::{debug, info};
use queue_models::probe_request::{ProbeRequest, TraceRequest, TraceRequestId};
use rand::prelude::*;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::analyse::EchoFollowUp;

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
    info!("Follow-up scheduler is ready for work.");
    loop {
        if let Some(req) = follow_up_rx.recv().await {
            probe_tx.send(flatten_request(req))?;
        } else {
            info!("Follow-up scheduler shutting down.");
            return Ok(());
        }
    }
}

fn flatten_request(req: FollowUpRequest) -> ProbeRequest {
    let id = req.id;
    let targets = if req.prefix_tree.confidence > 100 && thread_rng().gen_ratio(2, 3) {
        vec![] // if we have full confidence already, skip follow-ups in 2/3 of cases
    } else if req.prefix_tree.confidence > 60 && thread_rng().gen_ratio(1, 5) {
        vec![] // if we have more than 60% confidence, skip 20% of follow-ups
    } else {
        make_actual_targets(req) // actually push targets to the request
    };
    debug!(
        "Scheduling follow-up with {} remaining targets",
        targets.len()
    );

    ProbeRequest::Trace(TraceRequest { id, targets })
}

fn make_actual_targets(req: FollowUpRequest) -> Vec<Ipv6Addr> {
    let mut rng = thread_rng();
    req.follow_ups
        .into_iter()
        // drop 75% of requests to reduce load
        .filter(|_| rng.gen_ratio(1, 4))
        .flat_map(|it| it.targets)
        .collect()
}
