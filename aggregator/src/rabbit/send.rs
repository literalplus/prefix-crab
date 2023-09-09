use std::fmt::Debug;

use amqprs::channel::BasicPublishArguments;
use amqprs::BasicProperties;
use anyhow::{Context, Result};
use log::warn;
use prefix_crab::helpers::rabbit::RabbitHandle;
use prefix_crab::loop_with_stop;
use queue_models::probe_request::ProbeRequest;
use queue_models::RoutedMessage;
use serde::Serialize;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

struct RabbitSender<'han> {
    exchange_name: String,
    handle: &'han RabbitHandle,
    pretty_print: bool,
}

pub async fn run(
    handle: &RabbitHandle,
    work_rx: UnboundedReceiver<ProbeRequest>,
    exchange_name: String,
    pretty_print: bool,
    stop_rx: CancellationToken,
) -> Result<()> {
    RabbitSender {
        exchange_name,
        handle,
        pretty_print,
    }
    .run(work_rx, stop_rx)
    .await
    .with_context(|| "while sending RabbitMQ messages")
}

impl RabbitSender<'_> {
    async fn run(
        mut self,
        mut work_rx: UnboundedReceiver<ProbeRequest>,
        stop_rx: CancellationToken,
    ) -> Result<()> {
        loop_with_stop!(
            recv "probe sender", stop_rx,
            work_rx => do_send(it) on self
        )
    }

    async fn do_send(&mut self, msg: ProbeRequest) -> Result<()> {
        match self.publish(msg).await {
            Ok(_) => {}
            Err(e) => warn!("Failed to publish message: {:?}", e),
        }
        Ok(())
    }

    async fn publish(&self, msg: ProbeRequest) -> Result<()> {
        let args = BasicPublishArguments::new(&self.exchange_name, msg.routing_key());
        let bin = match msg {
            ProbeRequest::Echo(inner) => self.to_bin(&inner),
            ProbeRequest::Trace(inner) => self.to_bin(&inner),
        }?;
        self.handle
            .chan()
            .basic_publish(BasicProperties::default(), bin, args)
            .await
            .with_context(|| "during publish")?;
        Ok(())
    }

    fn to_bin(&self, msg: &(impl Serialize + Debug)) -> Result<Vec<u8>> {
        if self.pretty_print {
            serde_json::to_vec_pretty(&msg)
        } else {
            serde_json::to_vec(&msg)
        }
        .with_context(|| format!("during serialisation of {:?}", msg))
    }
}
