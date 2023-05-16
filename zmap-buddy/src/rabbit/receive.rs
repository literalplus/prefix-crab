use amqprs::channel::{BasicAckArguments, BasicConsumeArguments, ConsumerMessage};
use amqprs::Deliver;
// Cannot * due to Ok()
use anyhow::{Context, Result};
use log::{info, Level, log_enabled, trace, warn};
use tokio::sync::mpsc;

use crate::schedule;
use crate::schedule::TaskRequest;

use super::prepare::RabbitHandle;

struct RabbitReceiver<'han> {
    work_sender: mpsc::Sender<schedule::TaskRequest>,
    handle: &'han RabbitHandle,
}

pub async fn run(
    handle: &RabbitHandle,
    queue_name: String,
    work_sender: mpsc::Sender<schedule::TaskRequest>,
) -> Result<()> {
    RabbitReceiver { work_sender, handle }
        .run(queue_name)
        .await
        .with_context(|| "while listening for RabbitMQ messages")
}

impl RabbitReceiver<'_> {
    async fn run(mut self, queue_name: String) -> Result<()> {
        let mut rabbit_rx = self.start_consumer(&queue_name)
            .await?;
        let res = loop {
            let opt_msg = rabbit_rx.recv().await;
            match self.handle_msg(opt_msg).await {
                Ok(()) => {}
                Err(e) => break Err(e),
            }
        };
        drop(self.work_sender);
        res
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
                let request = TaskRequest {
                    model,
                    delivery_tag_to_ack: deliver.delivery_tag(),
                };
                self.work_sender.send(request)
                    .await
                    .with_context(|| "while passing received message")?;
            }
            Err(e) => {
                warn!(
                    "Unable to parse RabbitMQ message: {:?} - {:?} (ack to drop)",
                    e,
                    self.try_parse_utf8(content_slice)
                );
                self.ack(deliver.delivery_tag())
                    .await?
            }
        }
        Ok(())
    }

    fn try_parse_utf8<'a>(&'a self, content: &'a [u8]) -> &str {
        match std::str::from_utf8(content) {
            Ok(parsed) => parsed,
            Err(_) => "<< not UTF-8 >>",
        }
    }

    async fn ack(&self, delivery_tag: u64) -> Result<()> {
        self.handle.chan().basic_ack(BasicAckArguments::new(
            delivery_tag, false,
        )).await.with_context(|| "during immediate ack")?;
        Ok(())
    }
}
