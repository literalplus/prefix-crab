use amqprs::Deliver;
use anyhow::*;
use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::mpsc;

use prefix_crab::helpers::rabbit::receive::{self as helpers_receive, MessageHandler};
use queue_models::probe_request::TraceRequest;
use tokio_util::sync::CancellationToken;

use crate::schedule::TaskRequest;

use super::prepare::RabbitHandle;

pub async fn run(
    handle: &RabbitHandle,
    queue_name: String,
    work_sender: mpsc::Sender<TaskRequest>,
    stop_rx: CancellationToken,
) -> Result<()> {
    helpers_receive::run(
        handle, // not using a separate handle because the sender handles acks
        queue_name,
        TaskHandler { work_sender },
        stop_rx,
    )
    .await
}

struct TaskHandler {
    work_sender: mpsc::Sender<TaskRequest>,
}

#[async_trait]
impl MessageHandler for TaskHandler {
    type Model = TraceRequest;

    async fn handle_msg<'de>(&self, model: Self::Model, deliver: Deliver) -> Result<()>
    where
        Self::Model: Deserialize<'de>,
    {
        let request = TaskRequest {
            model,
            delivery_tag_to_ack: deliver.delivery_tag(),
        };
        self.work_sender
            .send(request)
            .await
            .with_context(|| "while passing received message")
    }

    fn consumer_tag() -> String {
        "yarrp-buddy trace request receiver".to_string()
    }
}
