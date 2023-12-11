use crate::handle_probe::TaskRequest;
use anyhow::*;
use clap::Args;
use log::debug;
use prefix_crab::helpers::rabbit::{ConfigureRabbit, RabbitHandle};
use prefix_crab::helpers::stop;
use queue_models::probe_request::ProbeRequest;
use queue_models::probe_response::{EchoProbeResponse, TraceResponse};
use queue_models::TypeRoutedMessage;
use tokio::select;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod receive;
mod send;

#[derive(Args, Debug)]
#[group(id = "rabbit")]
pub struct Params {
    /// URI for AMQP (RabbitMQ) server to connect to.
    /// Environment variable: AMQP_URI
    /// If a password is required, it is recommended to specify the URL over the environment or
    /// a config file, to avoid exposure in shell history and process list.
    #[arg(long, env = "AMQP_URI")]
    amqp_uri: String,

    /// Name of the queue to set up & listen to.
    #[arg(long, default_value = "prefix-crab.probe-response.aggregate")]
    in_queue_prefix: String,

    /// Name of the exchange to bind the queue to.
    #[arg(long, default_value = "prefix-crab.probe-response")]
    in_exchange_name: String,

    /// Name of the exchange to publish to.
    #[arg(long, default_value = "prefix-crab.probe-request")]
    out_exchange_name: String,

    /// Whether to pretty print JSON in RabbitMQ responses.
    #[arg(long, env = "PRETTY_PRINT")]
    pretty_print: bool,
}

impl Params {
    fn in_queue_name(&self, routing_key: &str) -> String {
        format!("{}-{}", self.in_queue_prefix, routing_key)
    }
}

pub async fn run(
    work_tx: mpsc::Sender<TaskRequest>,
    ack_rx: mpsc::Receiver<TaskRequest>,
    probe_rx: mpsc::Receiver<ProbeRequest>,
    stop_rx: CancellationToken,
    params: Params,
) -> Result<()> {
    let handle = prepare(&params).await?;
    let receiver = receive::run(&handle, &params, work_tx, stop_rx.clone(), ack_rx);
    let probe_sender = send::run(
        &handle,
        probe_rx,
        params.out_exchange_name.clone(),
        params.pretty_print,
        stop_rx,
    );
    let res = select! {
        exit_res = receiver => exit_res,
        exit_res = probe_sender => exit_res,
    };
    debug!("RabbitMQ handler is shutting down. Triggering clean stop.");
    stop::trigger();
    res
}

async fn prepare(params: &Params) -> Result<RabbitHandle> {
    let handle = RabbitHandle::connect(params.amqp_uri.as_str(), "aggregator").await?;
    let configure = ConfigureRabbit::new(&handle);

    configure
        .declare_exchange(&params.in_exchange_name, "direct")
        .await?;

    prepare_queue(&configure, params, TraceResponse::routing_key()).await?;
    prepare_queue(&configure, params, EchoProbeResponse::routing_key()).await?;

    configure
        .declare_exchange(&params.out_exchange_name, "direct")
        .await?;

    Ok(handle)
}

async fn prepare_queue(
    configure: &ConfigureRabbit<'_>,
    params: &Params,
    routing_key: &str,
) -> Result<()> {
    let queue_name = params.in_queue_name(routing_key);
    configure
        .declare_queue(&queue_name)
        .await?
        .bind_queue_routing(&queue_name, &params.in_exchange_name, routing_key)
        .await?;
    Ok(())
}
