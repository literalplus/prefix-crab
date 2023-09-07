use amqprs::BasicProperties;
use amqprs::channel::{BasicAckArguments, BasicPublishArguments};
use anyhow::{Context, Result};
use log::{info, warn};
use queue_models::RoutedMessage;
use tokio::select;
use tokio::sync::mpsc::UnboundedReceiver;

use queue_models::probe_response::EchoProbeResponse;
use tokio_util::sync::CancellationToken;

use crate::schedule::TaskResponse;

use super::prepare::RabbitHandle;

struct RabbitSender<'han> {
    work_receiver: UnboundedReceiver<TaskResponse>,
    exchange_name: String,
    handle: &'han RabbitHandle,
    pretty_print: bool,
}

pub async fn run(
    handle: &RabbitHandle,
    work_receiver: UnboundedReceiver<TaskResponse>,
    exchange_name: String,
    pretty_print: bool,
    stop_rx: CancellationToken,
) -> Result<()> {
    RabbitSender { work_receiver, exchange_name, handle, pretty_print }
        .run(stop_rx)
        .await
        .with_context(|| "while sending RabbitMQ messages")
}

impl RabbitSender<'_> {
    async fn run(mut self, stop_rx: CancellationToken) -> Result<()> {
        loop {
            let work_fut = self.work_receiver.recv();
            let stop_fut = stop_rx.cancelled();

            select! {
                biased; // Stop immediately
                _ = stop_fut => break Ok(()),
                msg_opt = work_fut => {
                    if let Some(msg) = msg_opt {
                        match self.do_send(msg).await {
                            Ok(_) => {}
                            Err(e) => warn!("Failed to publish/ack message: {:?}", e),
                        }
                    } else {
                        info!("Rabbit sender work channel was closed");
                        break Ok(());
                    }
                }
            }

        }
    }

    async fn do_send(&mut self, msg: TaskResponse) -> Result<()> {
        self.publish(msg.model).await?;
        self.ack(msg.acks_delivery_tag).await?;
        Ok(())
    }

    async fn publish(&self, msg: EchoProbeResponse) -> Result<()> {
        let args = BasicPublishArguments::new(&self.exchange_name, msg.routing_key());
        let bin = if self.pretty_print {
            serde_json::to_vec_pretty(&msg)
        } else {
            serde_json::to_vec(&msg)
        }.with_context(|| format!("during serialisation of {:?}", msg))?;
        self.handle.chan()
            .basic_publish(BasicProperties::default(), bin, args)
            .await
            .with_context(|| "during publish")?;
        Ok(())
    }

    async fn ack(&self, delivery_tag: u64) -> Result<()> {
        self.handle.chan().basic_ack(BasicAckArguments::new(
            delivery_tag, false,
        )).await.with_context(|| "during ack")?;
        Ok(())
    }
}
