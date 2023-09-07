use amqprs::channel::BasicAckArguments;
use anyhow::{Context, Result};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

use crate::loop_recv_with_stop;

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
    stop_rx: CancellationToken,
) -> Result<()>
where
    WorkType: CanAck,
{
    AckSender { handle }
        .run(work_receiver, stop_rx)
        .await
        .with_context(|| "while listening for RabbitMQ messages")
}

struct AckSender<'han> {
    handle: &'han RabbitHandle,
}

impl AckSender<'_> {
    async fn run(
        mut self,
        mut work_recv: UnboundedReceiver<impl CanAck>,
        stop_rx: CancellationToken,
    ) -> Result<()> {
        loop_recv_with_stop!(
            "ack sender", stop_rx, 
            work_recv => self.do_ack(it)
        );
    }

    async fn do_ack(&mut self, work: impl CanAck) -> Result<()> {
        // TODO support for rejects (after n retries maybe)?
        self.handle
            .chan()
            .basic_ack(BasicAckArguments {
                delivery_tag: work.delivery_tag(),
                multiple: false,
            })
            .await
            .with_context(|| "during ack")?;
        Ok(())
    }
}
