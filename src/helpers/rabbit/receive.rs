use amqprs::channel::{
    BasicConsumeArguments, BasicRejectArguments, ConsumerMessage,
};
use amqprs::Deliver;
// Cannot * due to Ok()
use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{log_enabled, trace, warn, Level};
use serde::Deserialize;
use serde_json;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::loop_recv_with_stop;

use super::RabbitHandle;

/// Runs a new JSON receiver. Note that this takes an owned handle because handles should not be
/// shared across threads/tasks. Use [RabbitHandle.clone] if you're on a shared reference, which
/// will open a fresh channel on the same connection.
pub async fn run<HandlerType>(
    handle: &RabbitHandle,
    queue_name: String,
    msg_handler: HandlerType,
    stop_rx: CancellationToken,
) -> Result<()>
where
    HandlerType: MessageHandler,
{
    JsonReceiver {
        handle,
        queue_name,
        msg_handler,
    }
    .run(stop_rx)
    .await
    .with_context(|| "while listening for RabbitMQ messages")
}

pub struct JsonReceiver<'han, HandlerType> {
    pub handle: &'han RabbitHandle,
    pub queue_name: String,
    pub msg_handler: HandlerType,
}

// Using trait because cannot store fn returning `impl Future` in struct
#[async_trait]
pub trait MessageHandler {
    type Model: for<'any_de> Deserialize<'any_de>;

    async fn handle_msg<'concrete_de>(&self, model: Self::Model, deliver: Deliver) -> Result<()>
    where
        Self::Model: Deserialize<'concrete_de>;

    fn consumer_tag() -> String;
}

impl<HandlerType: MessageHandler> JsonReceiver<'_, HandlerType> {
    pub async fn run(mut self, stop_rx: CancellationToken) -> Result<()> {
        let mut rabbit_rx = self.start_consumer().await?;

        // TODO implement recovery for channel closure (in macro probably)
        loop_recv_with_stop!(
            format!("receiver for {}", self.queue_name), stop_rx,
            rabbit_rx => self.handle_msg(it)
        );
    }

    async fn start_consumer(
        &self,
    ) -> Result<mpsc::UnboundedReceiver<ConsumerMessage>> {
        let consume_args =
            BasicConsumeArguments::new(&self.queue_name, &HandlerType::consumer_tag());
        let (_, rabbit_rx) = self
            .handle
            .chan()
            .basic_consume_rx(consume_args)
            .await
            .with_context(|| "while starting consumer")?;
        Ok(rabbit_rx)
    }

    async fn handle_msg(&mut self, msg: ConsumerMessage) -> Result<()> {
        // NOTE: By default, if a msg is un-ack'd for 30min, the consumer
        // is assumed faulty and the connection is closed with an error.
        // https://www.rabbitmq.com/consumers.html#acknowledgement-timeout
        let content = msg
            .content
            .expect("amqprs guarantees that received ConsumerMessage has content");
        let deliver = msg
            .deliver
            .expect("amqprs guarantees that received ConsumerMessage has deliver");
        self.parse_and_pass(content, deliver).await
    }

    async fn parse_and_pass(&mut self, content: Vec<u8>, deliver: Deliver) -> Result<()> {
        let content_slice = content.as_slice();
        if log_enabled!(Level::Trace) {
            trace!(
                "Got from RabbitMQ: {:?}",
                self.try_parse_utf8(content_slice)
            );
        }
        let parsed = serde_json::from_slice(content_slice);
        match parsed {
            Ok(model) => self
                .msg_handler
                .handle_msg(model, deliver)
                .await
                .with_context(|| "while handling message"),
            Err(e) => {
                warn!(
                    "Unable to parse RabbitMQ message: {:?} - {:?} (ack to drop)",
                    e,
                    self.try_parse_utf8(content_slice)
                );
                // We explicitly reject on a parsing error, everything else is not recoverable and
                // the messages will be anyways rejected due to channel disconnect
                self.reject_msg(deliver.delivery_tag()).await
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
        self.handle
            .chan()
            .basic_reject(BasicRejectArguments::new(
                delivery_tag,
                /* requeue = */ false,
            ))
            .await
            .with_context(|| "during immediate reject")?;
        Ok(())
    }
}
