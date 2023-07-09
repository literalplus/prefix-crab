use amqprs::channel::BasicAckArguments;
use anyhow::{Context, Result};
use log::debug;
use tokio::sync::mpsc::UnboundedReceiver;

use super::RabbitHandle;

pub trait CanAck {
    fn delivery_tag(&self) -> u64;
}

impl CanAck for u64 {
    fn delivery_tag(&self) -> u64 {
        *self
    }
}

pub async fn run<'de, WorkType>(
    handle: &RabbitHandle,
    work_receiver: UnboundedReceiver<WorkType>,
) -> Result<()> where WorkType: CanAck {
    AckSender { handle }
        .run(work_receiver)
        .await
        .with_context(|| "while listening for RabbitMQ messages")
}

struct AckSender<'han> {
    handle: &'han RabbitHandle,
}

impl AckSender<'_> {
    async fn run(mut self, mut work_recv: UnboundedReceiver<impl CanAck>) -> Result<()> {
        loop {
            if let Some(work) = work_recv.recv().await {
                self.do_ack(work).await?;
            } else {
                debug!("Stop sending acks because sender closed channel");
                return Ok(());
            }
        }
    }

    async fn do_ack(&mut self, work: impl CanAck) -> Result<()> {
        // TODO support for rejects (after n retries maybe)?
        self.handle.chan()
            .basic_ack(BasicAckArguments {
                delivery_tag: work.delivery_tag(),
                multiple: false,
            })
            .await
            .with_context(|| "during ack")?;
        Ok(())
    }
}
