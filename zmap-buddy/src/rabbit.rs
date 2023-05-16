use anyhow::*;
use clap::Args;
use tokio::select;
use tokio::sync::{broadcast, mpsc};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::schedule::{TaskRequest, TaskResponse};

/// Handles configuration and connection setup.
mod prepare;
mod receive;
mod send;

#[derive(Args)]
#[derive(Debug)]
#[group(id = "rabbit")]
pub struct Params {
    /// URI for AMQP (RabbitMQ) server to connect to.
    /// Environment variable: AMQP_URI
    /// If a password is required, it is recommended to specify the URL over the environment or
    /// a config file, to avoid exposure in shell history and process list.
    #[arg(long, default_value = "amqp://rabbit@10.45.87.51:5672/", env = "AMQP_URI")]
    amqp_uri: String,

    /// Name of the queue to set up & listen to.
    #[arg(long, default_value = "prefix-crab.probe-request.echo")]
    in_queue_name: String,

    /// Name of the exchange to bind the queue to.
    #[arg(long, default_value = "prefix-crab.probe-request")]
    in_exchange_name: String,

    /// Name of the exchange to publish to.
    #[arg(long, default_value = "prefix-crab.probe-response")]
    out_exchange_name: String,
}

pub async fn run(
    work_sender: mpsc::Sender<TaskRequest>,
    result_receiver: UnboundedReceiver<TaskResponse>,
    mut stop_rx: broadcast::Receiver<()>,
    params: Params,
) -> Result<()> {
    select! {
        biased; // Needed to handle stops immediately
        _ = stop_rx.recv() => Ok(()),
        exit_res = run_without_stop(work_sender, result_receiver, params) => exit_res,
    }
}

async fn run_without_stop(
    work_sender: mpsc::Sender<TaskRequest>,
    result_receiver: UnboundedReceiver<TaskResponse>,
    params: Params,
) -> Result<()> {
    let handle = prepare::prepare(&params)
        .await?;
    let sender = send::run(
        &handle, result_receiver, params.in_exchange_name,
    );
    let receiver = receive::run(
        &handle, params.in_queue_name, work_sender,
    );
    select! {
        exit_res = sender => exit_res,
        exit_res = receiver => exit_res,
    }
}
