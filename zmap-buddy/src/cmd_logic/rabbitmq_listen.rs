use std::sync::mpsc;
use std::time::Duration;
use anyhow::{Result, Context};
use clap::Args;
use log::info;

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

    // The receiver is sync, so using std instead of Tokio
    let (task_sender, task_receiver) = mpsc::channel();

    runtime.spawn_blocking(move || {
        // TODO zmap caller thread
        // TODO AtomicBool for shutdown on ctrl+c or SIGTERM
        // TODO For more complex situations in which it is desirable to stream data to or from the synchronous context, the mpsc channel has blocking_send and blocking_recv methods for use in non-async code such as the thread created by spawn_blocking.
        // ref: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html
        // TODO consider having this as a root-level Future, since we need to select! over the
        // task queue anyways, and then we could only have the actual zmap call be blocking..

        loop {
            if let Ok(received) = task_receiver.recv() {
                info!("Received something: {}", received);
            } else {
                info!("Looks like a shutdown");
                break;
            }
        }
        info!("bye from caller")
    });

    let request_handler_result = runtime.block_on(async {
        // TODO rabbitmq handler
        task_sender.send("hello (:")?;
        info!("oops done with the rabbit!");
        Ok::<(), anyhow::Error>(())
    }).with_context(|| "Request handler task existed abnormally"); // do NOT return here -> cleanup

    // TODO interrupt / sigterm handler
    drop(task_sender);

    info!("Shutting down with 15 seconds grace!");
    runtime.shutdown_timeout(Duration::from_secs(15));

    request_handler_result
}
