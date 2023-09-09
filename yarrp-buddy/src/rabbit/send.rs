use amqprs::channel::{BasicAckArguments, BasicPublishArguments};
use amqprs::BasicProperties;
use anyhow::{Context, Result};
use log::warn;
use prefix_crab::loop_with_stop;
use queue_models::RoutedMessage;
use tokio::sync::mpsc::UnboundedReceiver;

use queue_models::probe_response::TraceResponse;
use tokio_util::sync::CancellationToken;

use crate::schedule::TaskResponse;

use super::prepare::RabbitHandle;

struct RabbitSender<'han> {
    exchange_name: String,
    handle: &'han RabbitHandle,
    pretty_print: bool,
}

pub async fn run(
    handle: &RabbitHandle,
    work_rx: UnboundedReceiver<TaskResponse>,
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
        mut work_rx: UnboundedReceiver<TaskResponse>,
        stop_rx: CancellationToken,
    ) -> Result<()> {
        loop_with_stop! (
            recv "response sender", stop_rx,
            work_rx => do_send(it) on self
        )
    }

    async fn do_send(&mut self, msg: TaskResponse) -> Result<()> {
        let res = {
            self.publish(msg.model).await?;
            self.ack(msg.acks_delivery_tag).await
        };
        if let Err(e) = res {
            warn!("Error during publish/ack: {}", e);
        }
        Ok(())
    }

    async fn publish(&self, msg: TraceResponse) -> Result<()> {
        let args = BasicPublishArguments::new(&self.exchange_name, msg.routing_key());
        let bin = if self.pretty_print {
            serde_json::to_vec_pretty(&msg)
        } else {
            serde_json::to_vec(&msg)
        }
        .with_context(|| format!("during serialisation of {:?}", msg))?;
        self.handle
            .chan()
            .basic_publish(BasicProperties::default(), bin, args)
            .await
            .with_context(|| "during publish")?;
        Ok(())
    }

    async fn ack(&self, delivery_tag: u64) -> Result<()> {
        self.handle
            .chan()
            .basic_ack(BasicAckArguments::new(delivery_tag, false))
            .await
            .with_context(|| "during ack")?;
        Ok(())
    }
}
