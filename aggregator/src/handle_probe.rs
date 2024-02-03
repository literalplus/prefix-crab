use std::time::Instant;

use anyhow::*;
use clap::Args;
use db_model::prefix_tree::context::ContextFetchError;
use diesel::prelude::*;
use log::{error, info, trace};
use tokio::sync::mpsc::{Receiver, Sender};

use prefix_crab::{
    blocklist::{self, PrefixBlocklist}, drop_if_permanent, error::IsPermanent, helpers::rabbit::ack_sender::CanAck,
};
use queue_models::probe_response::ProbeResponse;

use crate::{analyse, schedule::FollowUpRequest};
mod archive;
mod echo;
mod trace;

#[derive(Args)]
#[group(id = "handleprobe")]
pub struct Params {
    #[clap(flatten)]
    pub blocklist: blocklist::Params,
}

#[derive(Debug)]
pub struct TaskRequest {
    pub model: ProbeResponse,
    pub received_at: Instant,
    pub delivery_tag: u64,
}

impl CanAck for TaskRequest {
    fn delivery_tag(&self) -> u64 {
        self.delivery_tag
    }
}

pub async fn run(
    task_rx: Receiver<TaskRequest>,
    ack_tx: Sender<TaskRequest>,
    follow_up_tx: Sender<FollowUpRequest>,
    params: Params,
) -> Result<()> {
    let conn = crate::persist::connect("aggregator - probe handler")?;
    let blocklist = blocklist::read(params.blocklist)?;
    let handler = ProbeHandler {
        conn,
        ack_tx,
        follow_up_tx,
        blocklist,
    };

    info!("Probe handler is ready to receive work!");
    handler.run(task_rx).await
}

struct ProbeHandler {
    conn: PgConnection,
    ack_tx: Sender<TaskRequest>,
    follow_up_tx: Sender<FollowUpRequest>,
    blocklist: PrefixBlocklist,
}

impl ProbeHandler {
    async fn run(mut self, mut task_rx: Receiver<TaskRequest>) -> Result<()> {
        loop {
            if let Some(req) = task_rx.recv().await {
                trace!("Received something: {:?}", req);
                self.handle_recv(req).await.context("handling probe responses")?;
            } else {
                info!("Probe handler shutting down.");
                return Ok(());
            }
        }
    }

    async fn handle_recv(&mut self, req: TaskRequest) -> Result<()> {
        match self.handle_one(&req).await {
            Result::Ok(_) => self.ack_tx.send(req).await.context("sending ack"),
            Err(e) => match process_error(e, &req) {
                Result::Ok(_) => {
                    self.ack_tx.send(req).await.context("sending err ack")?;
                    Ok(())
                }
                any => any,
            },
        }
    }

    async fn handle_one(&mut self, req: &TaskRequest) -> Result<()> {
        match &req.model {
            ProbeResponse::Echo(model) => self.handle_echo(model).await,
            ProbeResponse::Trace(model) => self.handle_trace(model),
        }
    }
}

fn process_error(e: Error, req: &TaskRequest) -> Result<()> {
    drop_if_permanent!(e <- ContextFetchError);
    drop_if_permanent!(e <- analyse::context::ContextFetchError);
    // TODO Could be handled with DLQ
    error!("Failed to handle request: {:?} - shutting down.", req);
    Err(e)
}