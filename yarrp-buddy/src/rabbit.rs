use anyhow::*;
use clap::Args;
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

use crate::schedule::{TaskRequest, TaskResponse};

/// Handles configuration and connection setup.
pub mod prepare;
mod receive;
mod send;

#[derive(Args, Clone, Debug)]
#[group(id = "rabbit")]
pub struct Params {
    /// URI for AMQP (RabbitMQ) server to connect to.
    /// Environment variable: AMQP_URI
    /// If a password is required, it is recommended to specify the URL over the environment or
    /// a config file, to avoid exposure in shell history and process list.
    #[arg(long, env = "AMQP_URI")]
    amqp_uri: String,

    /// Name of the queue to set up & listen to.
    #[arg(long, default_value = "prefix-crab.probe-request.trace")]
    in_queue_name: String,

    /// Name of the exchange to bind the queue to.
    #[arg(long, default_value = "prefix-crab.probe-request")]
    pub in_exchange_name: String,

    /// Name of the exchange to publish to.
    #[arg(long, default_value = "prefix-crab.probe-response")]
    out_exchange_name: String,

    /// Whether to pretty print JSON in RabbitMQ responses.
    #[arg(long, env = "PRETTY_PRINT")]
    pretty_print: bool,
}

pub async fn run(
    work_sender: mpsc::Sender<TaskRequest>,
    result_receiver: UnboundedReceiver<TaskResponse>,
    stop_rx: CancellationToken,
    params: Params,
) -> Result<()> {
    let handle = prepare::prepare(&params)
        .await?;
    let sender = send::run(
        &handle, result_receiver, params.out_exchange_name, params.pretty_print, stop_rx.clone(),
    );
    let receiver = receive::run(
        &handle, params.in_queue_name, work_sender, stop_rx,
    );
    select! {
        exit_res = sender => exit_res,
        exit_res = receiver => exit_res,
    }
}
