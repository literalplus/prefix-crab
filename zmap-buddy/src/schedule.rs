use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use log::{error, info, trace, warn};
use tokio::sync::mpsc::{Receiver, UnboundedSender};
use tokio_stream::{Stream, StreamExt};
use tokio_stream::wrappers::ReceiverStream;

use crate::schedule::task::SchedulerTask;
use crate::zmap_call;

pub use self::model::{ProbeResponse, TaskRequest, TaskResponse};

#[derive(Args)]
#[group(id = "scheduler")]
pub struct Params {
    #[clap(flatten)]
    base: zmap_call::Params,

    /// How long to wait for enough probes to arrive before flushing the chunk anyways
    /// and invoking zmap with less than the chunk size
    #[arg(long, default_value = "60", env = "CHUNK_TIMEOUT_SECS")]
    chunk_timeout_secs: u64,

    /// How many measurements to include in a chunk at most. If this many probes have been
    /// buffered, a chunk is immediately created and zmap will be invoked.
    #[arg(long, default_value = "16")]
    max_chunk_size: usize,
}

mod model;

const SAMPLES_PER_SUBNET: u16 = 16;

struct Scheduler {
    zmap_params: zmap_call::Params,
    result_tx: UnboundedSender<TaskResponse>,
}

pub async fn run(
    work_rx: Receiver<TaskRequest>,
    result_tx: UnboundedSender<TaskResponse>,
    params: Params,
) -> Result<()> {
    let work_stream = ReceiverStream::new(work_rx)
        .chunks_timeout(
            params.max_chunk_size,
            Duration::from_secs(params.chunk_timeout_secs),
        );
    Scheduler { zmap_params: params.base, result_tx }.run(work_stream).await
}

impl Scheduler {
    async fn run(&mut self, work_stream: impl Stream<Item=Vec<TaskRequest>>) -> Result<()> {
        let params = self.zmap_params.clone();
        tokio::task::spawn_blocking(move || {
            params.to_caller_verifying_sudo()?.verify_sudo_access()?;
            Ok::<(), anyhow::Error>(())
        }).await.with_context(|| "pre-flight sudo access check failed")??;
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

    async fn handle_scan_batch(&self, batch: Vec<TaskRequest>) {
        match self.do_scan_batch(batch).await {
            Ok(_) => info!("zmap call was successful."),
            Err(e) => {
                error!("zmap call failed: {}", e);
                // TODO signal this somehow
            }
        }
    }

    async fn do_scan_batch(&self, chunks: Vec<TaskRequest>) -> Result<()> {
        let mut task = SchedulerTask::new(self.zmap_params.clone())?;
        let mut at_least_one_ok = false;
        for chunk in chunks.iter() {
            match task.push_work(&chunk) {
                Err(e) => warn!("Unable to push work to task due to {}", e),
                Ok(_) => at_least_one_ok = true,
            }
        }
        if !at_least_one_ok {
            // TODO reconsider if this error handling makes sense
            return Err(anyhow!("None of the work in this chunk could be pushed successfully"));
        }
        let results = task.run().await?;
        for result in results {
            self.result_tx.send(result)
                .with_context(|| "while sending response over channel")?;
        }
        Ok(())
    }
}

mod task {
    use anyhow::{Context, Result};
    use log::trace;

    use prefix_crab::prefix_split::*;
    use crate::probe_store::{self, PrefixSplitProbeStore, PrefixStoreDispatcher, ProbeStore};
    use crate::zmap_call::{self, Caller, TargetCollector};

    use super::{TaskRequest, TaskResponse};

    pub struct SchedulerTask<'req> {
        store: PrefixSplitProbeStore<&'req TaskRequest>,
        caller: Caller,
        targets: TargetCollector,
    }

    impl<'req> SchedulerTask<'req> {
        pub fn new(zmap_params: zmap_call::Params) -> Result<Self> {
            Ok(Self {
                store: probe_store::create(),
                caller: zmap_params.to_caller_assuming_sudo()?,
                targets: TargetCollector::new_default()?,
            })
        }

        pub fn push_work(&mut self, item: &'req TaskRequest) -> Result<()> {
            self.push_work_internal(item).with_context(|| format!("for request: {:?}", item))
        }

        fn push_work_internal(&mut self, item: &'req TaskRequest) -> Result<()> {
            // TODO permute addresses
            let base_net = item.model.target_net;
            let split = split(base_net).context("splitting IPv6 prefix")?;
            let samples = split.to_samples(super::SAMPLES_PER_SUBNET);
            for sample in samples.iter() {
                self.targets.push_slice(sample.addresses.as_slice())?;
            }
            self.store.register_request(split, samples, &item);
            Ok(())
        }

        pub async fn run(mut self) -> Result<Vec<TaskResponse>> {
            let mut response_rx = self.caller.request_responses();
            self.targets.flush()?;
            let zmap_task = tokio::task::spawn_blocking(move || {
                trace!("Now calling zmap");
                self.caller.consume_run(self.targets)
            });
            let mut not_moved_store = self.store;
            while let Some(record) = response_rx.recv().await {
                trace!("response from zmap: {:?}", record);
                not_moved_store.register_response(&record);
            }
            response_rx.close(); // ensure nothing else is sent
            zmap_task.await.with_context(|| "during blocking zmap call (await)")??;
            not_moved_store.fill_missing();
            Ok(map_into_responses(not_moved_store))
        }
    }

    fn map_into_responses(store: PrefixSplitProbeStore<&TaskRequest>) -> Vec<TaskResponse> {
        store.stores.into_iter()
            .map(|it| map_into_response(it))
            .collect()
    }

    fn map_into_response(store: PrefixStoreDispatcher<&TaskRequest>) -> TaskResponse {
        let acks_delivery_tag = store.extra_data.delivery_tag_to_ack;
        TaskResponse { model: store.into(), acks_delivery_tag }
    }
}