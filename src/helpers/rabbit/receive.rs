use amqprs::channel::{BasicConsumeArguments, BasicRejectArguments, ConsumerMessage};
use amqprs::Deliver;
// Cannot * due to Ok()
use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{info, Level, log_enabled, trace, warn};
use serde::Deserialize;
use serde_json;
use tokio::sync::mpsc;

use super::RabbitHandle;

pub async fn run<'de, HandlerType>(
    handle: &RabbitHandle,
    queue_name: String,
    msg_handler: HandlerType,
) -> Result<()> where HandlerType: MessageHandler {
    JsonReceiver { handle, msg_handler }
        .run(queue_name)
        .await
        .with_context(|| "while listening for RabbitMQ messages")
}

struct JsonReceiver<'han, HandlerType> {
    handle: &'han RabbitHandle,
    msg_handler: HandlerType,
}

// Using trait because cannot store fn returning `impl Future` in struct
#[async_trait]
pub trait MessageHandler {
    type Model: for<'any_de> Deserialize<'any_de>;

    async fn handle_msg<'concrete_de>(
        &self, model: Self::Model, delivery_tag: u64,
    ) -> Result<()> where Self::Model: Deserialize<'concrete_de>;
}

impl<HandlerType: MessageHandler> JsonReceiver<'_, HandlerType> {
    async fn run(mut self, queue_name: String) -> Result<()> {
        let mut rabbit_rx = self.start_consumer(&queue_name)
            .await?;
        loop {
            let opt_msg = rabbit_rx.recv().await;
            match self.handle_msg(opt_msg).await {
                Ok(()) => {}
                Err(e) => break Err(e),
            }
        }
    }

    async fn start_consumer(
        &self, queue_name: &str,
    ) -> Result<mpsc::UnboundedReceiver<ConsumerMessage>> {
        let consume_args = BasicConsumeArguments::new(&queue_name, "zmap-buddy");
        let (_, rabbit_rx) = self.handle.chan()
            .basic_consume_rx(consume_args)
            .await
            .with_context(|| "while starting consumer")?;
        Ok(rabbit_rx)
    }

    async fn handle_msg(&mut self, opt_msg: Option<ConsumerMessage>) -> Result<()> {
        // NOTE: By default, if a msg is un-ack'd for 30min, the consumer
        // is assumed faulty and the connection is closed with an error.
        // https://www.rabbitmq.com/consumers.html#acknowledgement-timeout
        if let Some(msg) = opt_msg {
            let content = msg.content
                .expect("amqprs guarantees that received ConsumerMessage has content");
            let deliver = msg.deliver
                .expect("amqprs guarantees that received ConsumerMessage has deliver");
            self.parse_and_pass(content, deliver).await?;
        } else {
            info!("RabbitMQ channel was closed");
        }
        Ok(())
    }

    async fn parse_and_pass(&mut self, content: Vec<u8>, deliver: Deliver) -> Result<()> {
        let content_slice = content.as_slice();
        if log_enabled!(Level::Trace) {
            trace!("Got from RabbitMQ: {:?}", self.try_parse_utf8(content_slice));
        }
        let parsed = serde_json::from_slice(content_slice);
        match parsed {
            Ok(model) => {
                self.msg_handler.handle_msg(model, deliver.delivery_tag())
                    .await
                    .with_context(|| "while handling message")
            }
            Err(e) => {
                warn!(
                    "Unable to parse RabbitMQ message: {:?} - {:?} (ack to drop)",
                    e,
                    self.try_parse_utf8(content_slice)
                );
                // We explicitly reject on a parsing error, everything else is not recoverable and
                // the messages will be anyways rejected due to channel disconnect
                self.reject_msg(deliver.delivery_tag())
                    .await
            }
        }
    }

    fn try_parse_utf8<'a>(&'a self, content: &'a [u8]) -> &str {
        match std::str::from_utf8(content) {
            Ok(parsed) => parsed,
            Err(_) => "<< not UTF-8 >>",
        }
    }

    async fn reject_msg(&self, delivery_tag: u64) -> Result<()> {
        self.handle.chan().basic_reject(BasicRejectArguments::new(
            delivery_tag, /* requeue = */ false,
        )).await.with_context(|| "during immediate reject")?;
        Ok(())
    }
}
