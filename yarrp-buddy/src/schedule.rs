use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use log::{error, info, trace, warn};
use tokio::sync::mpsc::{Receiver, UnboundedSender};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};

use crate::schedule::task::SchedulerTask;
use crate::yarrp_call;

pub use self::model::{ProbeResponse, TaskRequest, TaskResponse};

mod task;
mod model;

#[derive(Args)]
#[group(id = "scheduler")]
pub struct Params {
    #[clap(flatten)]
    base: yarrp_call::Params,

    /// How long to wait for enough probes to arrive before flushing the chunk anyways
    /// and invoking zmap with less than the chunk size
    #[arg(long, default_value = "60", env = "CHUNK_TIMEOUT_SECS")]
    chunk_timeout_secs: u64,

    /// How many measurements to include in a chunk at most. If this many probes have been
    /// buffered, a chunk is immediately created and zmap will be invoked.
    #[arg(long, default_value = "32")]
    max_chunk_size: usize,
}

struct Scheduler {
    yarrp_params: yarrp_call::Params,
    result_tx: UnboundedSender<TaskResponse>,
}

pub async fn run(
    work_rx: Receiver<TaskRequest>,
    result_tx: UnboundedSender<TaskResponse>,
    params: Params,
) -> Result<()> {
    let work_stream = ReceiverStream::new(work_rx).chunks_timeout(
        params.max_chunk_size,
        Duration::from_secs(params.chunk_timeout_secs),
    );
    Scheduler {
        yarrp_params: params.base,
        result_tx,
    }
    .run(work_stream)
    .await
}

impl Scheduler {
    async fn run(&mut self, work_stream: impl Stream<Item = Vec<TaskRequest>>) -> Result<()> {
        let params = self.yarrp_params.clone();
        tokio::task::spawn_blocking(move || {
            params.to_caller_verifying_sudo()?.verify_sudo_access()?;
            Ok::<(), anyhow::Error>(())
        })
        .await
        .with_context(|| "pre-flight sudo access check failed")??;
        tokio::pin!(work_stream);
        info!("Scheduler up & running!");
        loop {
            if let Some(batch) = work_stream.next().await {
                trace!("Received something: {:?}", batch);
                self.handle_scan_batch(batch).await
            } else {
                info!("Scheduler shutting down.");
                return Ok(());
            }
        }
    }

    async fn handle_scan_batch(&self, batch: Vec<TaskRequest>) {
        match self.do_scan_batch(batch).await {
            Ok(_) => info!("Call was successful."),
            Err(e) => {
                error!("Call failed: {}", e);
                // TODO signal this somehow
            }
        }
    }

    async fn do_scan_batch(&self, chunks: Vec<TaskRequest>) -> Result<()> {
        let mut task = SchedulerTask::new(self.yarrp_params.clone())?;
        let mut at_least_one_ok = false;
        for chunk in chunks.into_iter() {
            match task.push_work(chunk) {
                Err(e) => warn!("Unable to push work to task {:?}", e),
                Ok(_) => at_least_one_ok = true,
            }
        }
        if !at_least_one_ok {
            // TODO reconsider if this error handling makes sense
            return Err(anyhow!(
                "None of the work in this chunk could be pushed successfully"
            ));
        }
        let results = task.run().await?;
        for result in results {
            self.result_tx
                .send(result)
                .with_context(|| "while sending response over channel")?;
        }
        Ok(())
    }
}
