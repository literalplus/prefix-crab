use anyhow::*;
use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::mpsc;

use prefix_crab::helpers::rabbit::receive::{self as helpers_receive, MessageHandler};
use queue_models::probe_request::EchoProbeRequest;

use crate::schedule::TaskRequest;

use super::prepare::RabbitHandle;

pub async fn run(
    handle: &RabbitHandle,
    queue_name: String,
    work_sender: mpsc::Sender<TaskRequest>,
) -> Result<()> {
    helpers_receive::run(handle, queue_name, TaskHandler { work_sender }).await
}

struct TaskHandler {
    work_sender: mpsc::Sender<TaskRequest>,
}

#[async_trait]
impl MessageHandler for TaskHandler {
    type Model = EchoProbeRequest;

    async fn handle_msg<'de>(
        &self, model: Self::Model, delivery_tag: u64,
    ) -> Result<()> where Self::Model: Deserialize<'de> {
        let request = TaskRequest {
            model,
            delivery_tag_to_ack: delivery_tag,
        };
        self.work_sender.send(request)
            .await
            .with_context(|| "while passing received message")
    }
}
