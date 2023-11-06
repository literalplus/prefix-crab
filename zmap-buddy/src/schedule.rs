use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use log::{error, info, trace, warn};
use prefix_crab::blocklist;
use tokio::sync::mpsc::{Receiver, UnboundedSender};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};

use crate::schedule::task::SchedulerTask;
use crate::zmap_call;

pub use self::model::{ProbeResponse, TaskRequest, TaskResponse};

mod interleave;
mod task;

#[derive(Args, Clone)]
#[group(id = "scheduler")]
pub struct Params {
    #[clap(flatten)]
    base: zmap_call::Params,

    #[clap(flatten)]
    blocklist: blocklist::Params,

    /// How long to wait for enough probes to arrive before flushing the chunk anyways
    /// and invoking zmap with less than the chunk size
    #[arg(long, default_value = "60", env = "CHUNK_TIMEOUT_SECS")]
    chunk_timeout_secs: u64,

    /// How many measurements to include in a chunk at most. If this many probes have been
    /// buffered, a chunk is immediately created and zmap will be invoked.
    #[arg(long, default_value = "16", env = "MAX_CHUNK_SIZE")]
    max_chunk_size: usize,
}

mod model;

const SAMPLES_PER_SUBNET: u16 = 16;

struct Scheduler {
    params: Params,
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
    Scheduler { params, result_tx }.run(work_stream).await
}

impl Scheduler {
    async fn run(&mut self, work_stream: impl Stream<Item = Vec<TaskRequest>>) -> Result<()> {
        self.check_preflight().await?;
        tokio::pin!(work_stream);
        info!("zmap scheduler up & running!");
        loop {
            if let Some(batch) = work_stream.next().await {
                trace!("Received something: {:?}", batch);
                self.handle_scan_batch(batch).await
            } else {
                info!("zmap scheduler shutting down.");
                return Ok(());
            }
        }
    }

    async fn check_preflight(&self) -> Result<()> {
        let params = self.params.clone();
        tokio::task::spawn_blocking(move || {
            params
                .base
                .to_caller_verifying_sudo()?
                .verify_sudo_access()?;
            Ok::<(), anyhow::Error>(())
        })
        .await
        .context("pre-flight sudo access check failed")??;
    info!(
        "Loading blocklist from `{:?}`",
        params.blocklist.blocklist_file
    );
        blocklist::read(params.blocklist)
            .context("pre-flight blocklist check failed")
            .map(|_| ())
    }

    async fn handle_scan_batch(&self, batch: Vec<TaskRequest>) {
        match self.do_scan_batch(batch).await {
            Ok(_) => info!("zmap call was successful."),
            Err(e) => {
                error!("zmap call failed: {}", e);
                // TODO signal this somehow (DLQ?)
            }
        }
    }

    async fn do_scan_batch(&self, batch: Vec<TaskRequest>) -> Result<()> {
        let mut task = SchedulerTask::new(self.params.clone())?;
        let mut at_least_one_ok = false;
        for item in batch.iter() {
            match task.push_work(item) {
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
