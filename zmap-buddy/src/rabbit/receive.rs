use amqprs::channel::{BasicAckArguments, BasicConsumeArguments, ConsumerMessage};
use amqprs::Deliver;
use anyhow::{Context, Result}; // Cannot * due to Ok()
use log::{info, trace};
use tokio::sync::mpsc;

use super::prepare::RabbitHandle;

struct RabbitReceiver<'han> {
    work_sender: mpsc::Sender<String>,
    handle: &'han RabbitHandle,
}

pub async fn run(
    handle: &RabbitHandle,
    queue_name: String,
    work_sender: mpsc::Sender<String>,
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
        let (_, rabbit_rx) = self.handle.borrow()
            .basic_consume_rx(consume_args)
            .await
            .with_context(|| "while starting consumer")?;
        Ok(rabbit_rx)
    }

    async fn handle_msg(&mut self, opt_msg: Option<ConsumerMessage>) -> Result<()> {
        if let Some(msg) = opt_msg {
            let content = msg.content
                .expect("amqprs guarantees that received ConsumerMessage has content");
            self.parse_and_pass(content).await?;
            let deliver = msg.deliver
                .expect("amqprs guarantees that received ConsumerMessage has deliver");
            self.ack(deliver).await?;
        } else {
            info!("RabbitMQ channel was closed");
        }
        Ok(())
    }

    async fn parse_and_pass(&mut self, content: Vec<u8>) -> Result<()> {
        let str_content = String::from_utf8(content)
            .with_context(|| "while parsing Rabbit message to UTF-8")?;
        trace!("got from rabbit: {:?}", str_content);
        self.work_sender.send(str_content)
            .await
            .with_context(|| "while passing received message")?;
        Ok(())
    }

    async fn ack(&self, deliver: Deliver) -> Result<()> {
        self.handle.borrow().basic_ack(BasicAckArguments::new(
            deliver.delivery_tag(), false,
        )).await.with_context(|| "during ack")?;
        Ok(())
    }
}
