use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args;
use futures::executor;
use log::debug;
use tokio::sync::mpsc;

#[derive(Args)]
pub struct Params {
    #[clap(flatten)]
    base: super::ZmapBaseParams,

    #[clap(flatten)]
    rabbit: rabbit_receiver::RabbitParams,
}

pub fn handle(params: Params) -> Result<()> {
    let (task_tx, task_rx) = mpsc::channel(4096);
    // This task if shut down by the RabbitMQ receiver closing the channel
    tokio::spawn(zmap_queue::start_handler(
        task_rx, params.base.to_caller()?,
    ));

    let sig_handler = signal_handler::new();
    let stop_rx = sig_handler.subscribe_stop();
    tokio::spawn(sig_handler.wait_for_signal());

    let receiver_result = executor::block_on(rabbit_receiver::start_receiver(
        task_tx, stop_rx, params.rabbit,
    ));

    receiver_result
}

mod signal_handler {
    use log::{info, warn};
    use tokio::select;
    use tokio::signal::unix::{signal, SignalKind};
    use tokio::sync::broadcast;

    pub struct SignalHandler {
        stop_tx: broadcast::Sender<()>,
    }

    pub fn new() -> SignalHandler {
        let (stop_tx, _) = broadcast::channel(1);
        SignalHandler { stop_tx }
    }

    impl SignalHandler {
        pub fn subscribe_stop(&self) -> broadcast::Receiver<()> {
            self.stop_tx.subscribe()
        }

        pub async fn wait_for_signal(self) {
            let mut sigterm = signal(SignalKind::terminate()).unwrap();
            let mut sigint = signal(SignalKind::interrupt()).unwrap();
            loop {
                select! {
                _ = sigterm.recv() => info!("Terminated; stopping..."),
                _ = sigint.recv() => info!("Interrupted; stopping..."),
                }
                if let Err(e) = self.stop_tx.send(()) {
                    warn!("Failed to notify tasks to stop, maybe they're already finished. {}", e);
                }
                break;
            }
        }
    }
}

mod rabbit_receiver {
    use amqprs::channel::{BasicAckArguments, BasicConsumeArguments, Channel, ConsumerMessage, QueueDeclareArguments};
    use amqprs::connection::{Connection, OpenConnectionArguments};
    use amqprs::Deliver;
    use anyhow::{bail, Context, Result};
    use clap::Args;
    use log::{debug, info, trace};
    use tokio::select;
    use tokio::sync::{broadcast, mpsc};

    #[derive(Args)]
    #[derive(Debug)]
    pub struct RabbitParams {
        /// URI for AMQP (RabbitMQ) server to connect to.
        /// Environment variable: AMQP_URI
        /// If a password is required, it is recommended to specify the URL over the environment or
        /// a config file, to avoid exposure in shell history and process list.
        #[arg(long, default_value = "amqp://rabbit@10.45.87.51:5672/", env = "AMQP_URI")]
        amqp_uri: String,

        /// Name of the queue to set up & listen to.
        #[arg(long, default_value = "prefix-crab.probe-request.echo")]
        queue_name: String,
    }

    struct RabbitReceiver {
        work_sender: mpsc::Sender<String>,
        stop_rx: broadcast::Receiver<()>,
        channel: Channel,
    }

    pub async fn start_receiver(
        work_sender: mpsc::Sender<String>,
        mut stop_rx: broadcast::Receiver<()>,
        rabbit_params: RabbitParams,
    ) -> Result<()> {
        let prepare_fut = prepare(&rabbit_params);
        let (_connection, channel) = select! {
            biased;
            _ = stop_rx.recv() => bail!("Interrupted during RabbitMQ setup"),
            res = prepare_fut => {
                res.with_context(|| "while setting up RabbitMQ")?
            }
        };

        RabbitReceiver { work_sender, stop_rx, channel }
            .run(rabbit_params.queue_name)
            .await
            .with_context(|| "while listening for RabbitMQ messages")
    }

    async fn prepare(rabbit_params: &RabbitParams) -> Result<(Connection, Channel)> {
        debug!("Setting up RabbitMQ receiver...");
        let connection_args = OpenConnectionArguments::try_from(
            rabbit_params.amqp_uri.as_str(),
        ).with_context(|| format!("Invalid connection URI in {:?}", rabbit_params))?;
        let connection = Connection::open(&connection_args)
            .await
            .with_context(|| format!("while opening RabbitMQ connection {:?}", rabbit_params))?;
        let channel = connection.open_channel(None)
            .await
            .with_context(|| "while opening RabbitMQ channel")?;
        let mut queue_args =
            QueueDeclareArguments::new(rabbit_params.queue_name.as_str());
        queue_args.durable(true);
        channel.queue_declare(queue_args)
            .await
            .with_context(|| "while declaring queue")?
            .expect("queue_declare returned None even though no_wait was false");
        Ok((connection, channel))
    }

    impl RabbitReceiver {
        async fn run(mut self, queue_name: String) -> Result<()> {
            let mut rabbit_rx = self.start_consumer(&queue_name).await?;
            let res = loop {
                select! {
                    biased;
                    _ = self.stop_rx.recv() => {
                        break Ok(());
                    },
                    opt_msg = rabbit_rx.recv() => {
                        match self.handle_msg(opt_msg).await {
                            Ok(()) => {},
                            Err(e) => break Err(e),
                        }
                    }
                }
            };
            drop(self.work_sender);
            res
        }

        async fn start_consumer(
            &self, queue_name: &str,
        ) -> Result<mpsc::UnboundedReceiver<ConsumerMessage>> {
            let consume_args = BasicConsumeArguments::new(&queue_name, "zmap-buddy");
            let (_, rabbit_rx) = self.channel.basic_consume_rx(consume_args)
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
            self.channel.basic_ack(BasicAckArguments::new(
                deliver.delivery_tag(), false,
            )).await.with_context(|| "during ack")?;
            Ok(())
        }
    }
}

mod zmap_queue {
    use log::info;
    use tokio::sync::mpsc::Receiver;
    use crate::zmap_call::Caller;

    struct QueueHandler {
        work_receiver: Receiver<String>,
    }

    pub async fn start_handler(work_receiver: Receiver<String>, caller: Caller) {
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
