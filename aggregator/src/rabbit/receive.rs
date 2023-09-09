use std::marker::PhantomData;

use amqprs::Deliver;
use anyhow::*;
use async_trait::async_trait;
use prefix_crab::helpers::rabbit::ack_sender::AckSender;
use prefix_crab::loop_with_stop;
use queue_models::TypeRoutedMessage;
use serde::Deserialize;
use tokio::select;
use tokio::sync::mpsc;

use prefix_crab::helpers::rabbit::receive::{
    JsonReceiver, MessageHandler,
};
use prefix_crab::helpers::rabbit::RabbitHandle;
use queue_models::probe_response::{EchoProbeResponse, ProbeResponse, TraceResponse};
use tokio_util::sync::CancellationToken;

use crate::handle_probe::TaskRequest;

use super::Params;

pub async fn run(
    handle: &RabbitHandle,
    params: &Params,
    work_tx: mpsc::Sender<TaskRequest>,
    stop_rx: CancellationToken,
    ack_rx: mpsc::UnboundedReceiver<TaskRequest>,
) -> Result<()> {
    let echo_handle = handle.fork().await?;
    let echo_recv = make_receiver::<EchoProbeResponse>(&echo_handle, work_tx.clone(), params);
    let trace_handle = handle.fork().await?;
    let trace_recv = make_receiver::<TraceResponse>(&trace_handle, work_tx, params);

    let ack = run_ack_router(&echo_handle, &trace_handle, ack_rx, stop_rx.clone());
    let trace = trace_recv.run(stop_rx.clone());
    let echo = echo_recv.run(stop_rx);

    select! {
        res = echo => res.context("in echo listener"),
        res = trace => res.context("in trace listener"),
        res = ack => res.context("in ack sender"),
    }
}

fn make_receiver<'han, T>(
    handle: &'han RabbitHandle,
    work_sender: mpsc::Sender<TaskRequest>,
    params: &Params,
) -> JsonReceiver<'han, ResponseHandler<T>>
where
    T: TypeRoutedMessage + Into<ProbeResponse> + for<'a> Deserialize<'a> + Send + Sync,
{
    JsonReceiver {
        handle,
        msg_handler: ResponseHandler {
            work_sender,
            marker: PhantomData::<T>,
        },
        queue_name: params.in_queue_name(T::routing_key()),
    }
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

async fn run_ack_router(
    echo_handle: &RabbitHandle, trace_handle: &RabbitHandle,
    ack_rx: mpsc::UnboundedReceiver<TaskRequest>, stop_rx: CancellationToken
) -> Result<()> {
    AckRouter {
        echo_ack: AckSender::new(echo_handle),
        trace_ack: AckSender::new(trace_handle),
    }
    .run(ack_rx, stop_rx.clone()).await
}

// Struct needed to pass the two senders to the handler function (macro doesn't support that)
struct AckRouter<'a, 'b> {
    echo_ack: AckSender<'a>,
    trace_ack: AckSender<'b>,
}

impl AckRouter<'_, '_> {
    async fn run(
        mut self,
        mut ack_rx: mpsc::UnboundedReceiver<TaskRequest>,
        stop_rx: CancellationToken,
    ) -> Result<()> {
        loop_with_stop!(
            recv "ack router", stop_rx,
            ack_rx => route_ack(it) on self
        )
    }

    async fn route_ack(&mut self, work: TaskRequest) -> Result<()> {
        use ProbeResponse as R;

        match work.model {
            R::Echo(_) => self.echo_ack.do_ack(work).await,
            R::Trace(_) => self.trace_ack.do_ack(work).await,
        }
    }
}
