use anyhow::*;
use clap::Args;
use tokio::select;
use tokio::sync::{broadcast, mpsc};
use prefix_crab::helpers::rabbit::{ConfigureRabbit, RabbitHandle};
use crate::handle_probe::TaskRequest;

mod receive;

#[derive(Args)]
#[derive(Debug)]
#[group(id = "rabbit")]
pub struct Params {
    /// URI for AMQP (RabbitMQ) server to connect to.
    /// Environment variable: AMQP_URI
    /// If a password is required, it is recommended to specify the URL over the environment or
    /// a config file, to avoid exposure in shell history and process list.
    #[arg(long, default_value = "amqp://rabbit:localsetupveryinsecure@10.45.87.51:5672/", env = "AMQP_URI")]
    amqp_uri: String,

    /// Name of the queue to set up & listen to.
    #[arg(long, default_value = "prefix-crab.probe-response.aggregate")]
    in_queue_name: String,

    /// Name of the exchange to bind the queue to.
    #[arg(long, default_value = "prefix-crab.probe-response")]
    in_exchange_name: String,
}

pub async fn run(
    work_sender: mpsc::Sender<TaskRequest>,
    mut stop_rx: broadcast::Receiver<()>,
    params: Params,
) -> Result<()> {
    select! {
        biased; // Needed to handle stops immediately
        _ = stop_rx.recv() => Ok(()),
        exit_res = run_without_stop(work_sender, params) => exit_res,
    }
}

async fn run_without_stop(
    work_sender: mpsc::Sender<TaskRequest>,
    params: Params,
) -> Result<()> {
    let handle = prepare(&params)
        .await?;
    receive::run(
        &handle, params.in_queue_name, work_sender,
    ).await
}

async fn prepare(params: &Params) -> Result<RabbitHandle> {
    let handle = RabbitHandle::connect(params.amqp_uri.as_str())
        .await?;

    let queue_name = params.in_queue_name.as_str();
    let in_exchange_name = params.in_exchange_name.as_str();
    let configure = ConfigureRabbit::new(&handle);

    configure
        .declare_queue(queue_name).await?
        .bind_queue_to(queue_name, in_exchange_name).await?;

    Ok(handle)
}
