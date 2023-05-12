use anyhow::*;
use clap::Args;
use tokio::select;
use tokio::sync::{broadcast, mpsc};

/// Handles configuration and connection setup.
mod prepare;
mod receive;

mod send {
    use anyhow::*;

    use super::prepare::RabbitHandle;

    pub async fn run(
        _handle: &RabbitHandle,
        _exchange_name: String,
    ) -> Result<()> {
        // FIXME implement
        Ok(())
    }
}


#[derive(Args)]
#[derive(Debug)]
pub struct RabbitParams {
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
    work_sender: mpsc::Sender<String>,
    mut stop_rx: broadcast::Receiver<()>,
    params: RabbitParams,
) -> Result<()> {
    //let inner_run = run_without_stop(work_sender, params);
    select! {
        biased; // Needed to handle stops immediately
        _ = stop_rx.recv() => Ok(()),
        exit_res = run_without_stop(work_sender, params) => exit_res,
    }
}

async fn run_without_stop(
    work_sender: mpsc::Sender<String>,
    params: RabbitParams,
) -> Result<()> {
    let handle = prepare::prepare(&params)
        .await?;
    let sender = send::run(&handle, params.in_exchange_name);
    let receiver = receive::run(
        &handle, params.in_queue_name, work_sender,
    );
    select! {
        exit_res = sender => exit_res,
        exit_res = receiver => exit_res,
    }
}
