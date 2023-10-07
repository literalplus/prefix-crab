use anyhow::*;
use db_model::prefix_tree::context::ContextFetchError;
use diesel::prelude::*;
use log::{error, info, trace};
use tokio::sync::mpsc::{Receiver, UnboundedSender};

use prefix_crab::{helpers::rabbit::ack_sender::CanAck, error::IsPermanent, drop_if_permanent};
use queue_models::probe_response::ProbeResponse;

use crate::{schedule::FollowUpRequest, analyse};
mod archive;
mod echo;
mod trace;

#[derive(Debug)]
pub struct TaskRequest {
    pub model: ProbeResponse,
    pub delivery_tag: u64,
}

impl CanAck for TaskRequest {
    fn delivery_tag(&self) -> u64 {
        self.delivery_tag
    }
}

pub async fn run(
    task_rx: Receiver<TaskRequest>,
    ack_tx: UnboundedSender<TaskRequest>,
    follow_up_tx: UnboundedSender<FollowUpRequest>,
) -> Result<()> {
    let conn = crate::persist::connect()?;
    let handler = ProbeHandler {
        conn,
        ack_tx,
        follow_up_tx,
    };

    info!("Probe handler is ready to receive work!");
    handler.run(task_rx).await
}

struct ProbeHandler {
    conn: PgConnection,
    ack_tx: UnboundedSender<TaskRequest>,
    follow_up_tx: UnboundedSender<FollowUpRequest>,
}

impl ProbeHandler {
    async fn run(mut self, mut task_rx: Receiver<TaskRequest>) -> Result<()> {
        loop {
            if let Some(req) = task_rx.recv().await {
                trace!("Received something: {:?}", req);
                self.handle_recv(req).context("handling probe responses")?;
            } else {
                info!("Probe handler shutting down.");
                return Ok(());
            }
        }
    }

    fn handle_recv(&mut self, req: TaskRequest) -> Result<()> {
        match self.handle_one(&req) {
            Result::Ok(_) => self.ack_tx.send(req).context("sending ack"),
            Err(e) => {
                drop_if_permanent!(e <- ContextFetchError);
                drop_if_permanent!(e <- analyse::context::ContextFetchError);
                // TODO Could be handled with DLQ
                error!("Failed to handle request: {:?} - shutting down.", req);
                Err(e)
            }
        }
    }

    fn handle_one(&mut self, req: &TaskRequest) -> Result<()> {
        match &req.model {
            ProbeResponse::Echo(model) => self.handle_echo(model),
            ProbeResponse::Trace(model) => self.handle_trace(model),
        }
    }
}
