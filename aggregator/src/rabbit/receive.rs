use std::marker::PhantomData;

use amqprs::Deliver;
use anyhow::*;
use async_trait::async_trait;
use queue_models::TypeRoutedMessage;
use serde::Deserialize;
use tokio::select;
use tokio::sync::mpsc;

use prefix_crab::helpers::rabbit::receive::{self as helpers_receive, MessageHandler};
use prefix_crab::helpers::rabbit::RabbitHandle;
use queue_models::probe_response::{EchoProbeResponse, ProbeResponse, TraceResponse};
use tokio_util::sync::CancellationToken;

use crate::handle_probe::TaskRequest;

use super::Params;

pub async fn run(
    handle: &RabbitHandle,
    params: &Params,
    work_sender: mpsc::Sender<TaskRequest>,
    stop_rx: CancellationToken,
) -> Result<()> {
    let echo = run_receiver::<EchoProbeResponse>(
        handle, params, work_sender.clone(), stop_rx.clone(),
    );
    let trace = run_receiver::<TraceResponse>(
        handle, params, work_sender, stop_rx,
    );
    select! {
        res = echo => res.context("in echo listener"),
        res = trace => res.context("in trace listener"),
    }
}

async fn run_receiver<T>(
    handle: &RabbitHandle,
    params: &Params,
    work_sender: mpsc::Sender<TaskRequest>,
    stop_rx: CancellationToken,
) -> Result<()>
where
    T: TypeRoutedMessage + Into<ProbeResponse> + for<'a> Deserialize<'a> + Send + Sync,
{
    helpers_receive::run(
        handle.fork().await?,
        params.in_queue_name(T::routing_key()),
        ResponseHandler {
            work_sender,
            marker: PhantomData::<T>,
        },
        stop_rx,
    )
    .await
}

struct ResponseHandler<T: Into<ProbeResponse>> {
    work_sender: mpsc::Sender<TaskRequest>,
    marker: PhantomData<T>,
}

#[async_trait]
impl<T> MessageHandler for ResponseHandler<T>
where
    T: Into<ProbeResponse> + for<'a> Deserialize<'a> + Send + Sync + TypeRoutedMessage,
{
    type Model = T;

    async fn handle_msg<'de>(&self, model: Self::Model, deliver: Deliver) -> Result<()> {
        let request = TaskRequest {
            model: model.into(),
            delivery_tag: deliver.delivery_tag(),
        };
        self.work_sender
            .send(request)
            .await
            .with_context(|| "while passing received message")
    }

    fn consumer_tag() -> String {
        format!("aggregator {} response receiver", T::routing_key())
    }
}
