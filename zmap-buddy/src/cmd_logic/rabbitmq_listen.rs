use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args;
use log::info;
use tokio::sync::mpsc;

#[derive(Args)]
pub struct Params {
    #[clap(flatten)]
    base: super::ZmapBaseParams,

    #[arg(default_value = "amqp://10.45.87.51:5672/")]
    amqp_url: String,

    #[arg(default_value = "prefix-crab.probe-request.echo")]
    queue_name: String,
}

pub fn handle(params: Params) -> Result<()> {
    // FIXME

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .with_context(|| "Failed to start Tokio runtime")?;

    let (task_sender, task_receiver) = mpsc::channel(4096);
    runtime.spawn(async move {
        zmap_queue::start_handler(task_receiver).await
    });

    let receiver_result = runtime.block_on(async move {
        rabbit_receiver::start_receiver(
            task_sender, "amqp://10.45.87.51:5672/",
        ).await
    }).with_context(|| "in Rabbit receiver"); // do NOT return here -> cleanup

    // TODO interrupt / sigterm handler

    info!("Shutting down with 15 seconds grace!");
    runtime.shutdown_timeout(Duration::from_secs(15));

    receiver_result
}

mod rabbit_receiver {
    use amqprs::channel::QueueDeclareArguments;
    use amqprs::connection::Connection;
    use amqprs::connection::OpenConnectionArguments;
    use anyhow::{Context, Error};
    use log::info;
    use tokio::sync::mpsc::Sender;

    struct RabbitReceiver {
        work_sender: Sender<String>,
    }

    pub async fn start_receiver(
        work_sender: Sender<String>, rabbit_url: &str,
    ) -> Result<(), Error> {
        let connection_args = OpenConnectionArguments::try_from(rabbit_url)
            .with_context(|| format!("Invalid connection URI {}", rabbit_url))?;
        let connection = Connection::open(&connection_args)
            .await
            .with_context(|| format!("while opening RabbitMQ connection {}", rabbit_url))?;
        let channel = connection.open_channel(None)
            .await
            .with_context(|| "while opening RabbitMQ channel")?;
        let mut queue_args = QueueDeclareArguments::new("prefix-crab.probe-request.echo");
        queue_args.durable(true);
        let (queue_name, _, _) = channel
            .queue_declare(queue_args)
            .await
            .with_context(|| "while declaring queue")?
            .expect("queue_declare returned None even though no_wait was false");
        RabbitReceiver { work_sender }.run().await
    }

    impl RabbitReceiver {
        async fn run(self) -> Result<(), Error> {
            // TODO rabbitmq handler
            self.work_sender.send("hello (:".to_string())
                .await
                .with_context(|| "while sending work")?;
            info!("oops done with the rabbit!");
            drop(self.work_sender);
            Ok(())
        }
    }
}

mod zmap_queue {
    use log::info;
    use tokio::sync::mpsc::Receiver;

    struct QueueHandler {
        work_receiver: Receiver<String>,
    }

    pub async fn start_handler(work_receiver: Receiver<String>) {
        QueueHandler { work_receiver }.run().await
    }

    impl QueueHandler {
        async fn run(mut self) {
            // TODO zmap caller thread
            // TODO AtomicBool for shutdown on ctrl+c or SIGTERM
            // TODO For more complex situations in which it is desirable to stream data to or from the synchronous context, the mpsc channel has blocking_send and blocking_recv methods for use in non-async code such as the thread created by spawn_blocking.
            // ref: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html
            // TODO consider having this as a root-level Future, since we need to select! over the
            // task queue anyways, and then we could only have the actual zmap call be blocking..

            loop {
                if let Some(received) = self.work_receiver.recv().await {
                    info!("Received something: {}", received);
                    // TODO spawn zmap caller with spawn_blocking
                } else {
                    info!("Looks like a shutdown");
                    break;
                }
            }
            info!("bye from caller")
        }
    }
}
