use amqprs::channel::BasicAckArguments;
use anyhow::*;
use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::mpsc;

use prefix_crab::helpers::rabbit::RabbitHandle;
use prefix_crab::helpers::rabbit::receive::{self as helpers_receive, MessageHandler};
use queue_models::echo_response::EchoProbeResponse;

use crate::handle_probe::TaskRequest;

pub async fn run(
    handle: &RabbitHandle,
    queue_name: String,
    work_sender: mpsc::Sender<TaskRequest>,
) -> Result<()> {
    helpers_receive::run(handle, queue_name, TaskHandler { work_sender, handle }).await
}

struct TaskHandler<'han> {
    work_sender: mpsc::Sender<TaskRequest>,
    handle: &'han RabbitHandle,
}

#[async_trait]
impl MessageHandler for TaskHandler<'_> {
    type Model = EchoProbeResponse;

    async fn handle_msg<'de>(
        &self, model: Self::Model, delivery_tag: u64,
    ) -> Result<()> where Self::Model: Deserialize<'de> {
        let request = TaskRequest { model };
        // TODO ack only when processed...
        self.handle.chan()
            .basic_ack(BasicAckArguments { delivery_tag, multiple: false })
            .await
            .with_context(|| "during ack")?;
        self.work_sender.send(request)
            .await
            .with_context(|| "while passing received message")
    }
}
