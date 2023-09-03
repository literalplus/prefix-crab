use std::fmt::Debug;

use amqprs::BasicProperties;
use amqprs::channel::BasicPublishArguments;
use anyhow::{Context, Result};
use log::{info, warn};
use prefix_crab::helpers::rabbit::RabbitHandle;
use queue_models::RoutedMessage;
use queue_models::probe_request::ProbeRequest;
use serde::Serialize;
use tokio::sync::mpsc::UnboundedReceiver;

struct RabbitSender<'han> {
    work_receiver: UnboundedReceiver<ProbeRequest>,
    exchange_name: String,
    handle: &'han RabbitHandle,
    pretty_print: bool,
}

pub async fn run(
    handle: &RabbitHandle,
    work_receiver: UnboundedReceiver<ProbeRequest>,
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
                    Err(e) => warn!("Failed to publish message: {:?}", e),
                }
            } else {
                info!("Rabbit sender work channel was closed");
                break Ok(());
            }
        }
    }

    async fn do_send(&mut self, msg: ProbeRequest) -> Result<()> {
        self.publish(msg).await?;
        Ok(())
    }

    async fn publish(&self, msg: ProbeRequest) -> Result<()> {
        let args = BasicPublishArguments::new(&self.exchange_name, msg.routing_key());
        let bin = match msg {
            ProbeRequest::Echo(inner) => self.to_bin(&inner),
            ProbeRequest::Trace(inner) => self.to_bin(&inner),
        }?;
        self.handle.chan()
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
        }.with_context(|| format!("during serialisation of {:?}", msg))
    }
}
