use amqprs::BasicProperties;
use amqprs::channel::{BasicAckArguments, BasicPublishArguments};
use anyhow::{Context, Result};
use log::{info, warn};
use tokio::sync::mpsc::UnboundedReceiver;

use queue_models::echo_response::EchoProbeResponse;

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
) -> Result<()> {
    RabbitSender { work_receiver, exchange_name, handle, pretty_print }
        .run()
        .await
        .with_context(|| "while sending RabbitMQ messages")
}

impl RabbitSender<'_> {
    async fn run(mut self) -> Result<()> {
        loop {
            if let Some(msg) = self.work_receiver.recv().await {
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

    async fn do_send(&mut self, msg: TaskResponse) -> Result<()> {
        self.publish(msg.model).await?;
        self.ack(msg.acks_delivery_tag).await?;
        Ok(())
    }

    async fn publish(&self, msg: EchoProbeResponse) -> Result<()> {
        let args = BasicPublishArguments::new(&self.exchange_name, "");
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
