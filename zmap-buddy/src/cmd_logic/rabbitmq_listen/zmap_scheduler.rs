use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use ipnet::Ipv6Net;
use log::{error, info, trace, warn};
use tokio::sync::mpsc::Receiver;
use tokio_stream::{Stream, StreamExt};
use tokio_stream::wrappers::ReceiverStream;

use queue_models::echo_response::EchoProbeResponse;

use crate::cmd_logic::ZmapBaseParams;
use crate::prefix_split;
use crate::probe_store::{self, PrefixSplitProbeStore, ProbeStore};
use crate::zmap_call::{Caller, TargetCollector};

#[derive(Args)]
pub struct SchedulerParams {
    #[clap(flatten)]
    base: ZmapBaseParams,

    /// How long to wait for enough probes to arrive before flushing the chunk anyways
    /// and invoking zmap with less than the chunk size
    #[arg(long, default_value = "60")]
    chunk_timeout_secs: u64,

    /// How many measurements to include in a chunk at most. If this many probes have been
    /// buffered, a chunk is immediately created and zmap will be invoked.
    #[arg(long, default_value = "16")]
    max_chunk_size: usize,
}

struct Scheduler {
    zmap_params: ZmapBaseParams,
}

pub async fn start(
    work_rx: Receiver<String>,
    params: SchedulerParams,
) -> Result<()> {
    let work_stream = ReceiverStream::new(work_rx)
        .chunks_timeout(
            params.max_chunk_size,
            Duration::from_secs(params.chunk_timeout_secs),
        );
    Scheduler { zmap_params: params.base }.run(work_stream).await
}

impl Scheduler {
    async fn run(&mut self, work_stream: impl Stream<Item=Vec<String>>) -> Result<()> {
        let params = self.zmap_params.clone();
        tokio::task::spawn_blocking(move || {
            params.to_caller_verifying_sudo()?.verify_sudo_access()?;
            Ok::<(), anyhow::Error>(())
        }).await.with_context(|| "pre-flight sudo access check failed")??;
        tokio::pin!(work_stream);
        info!("zmap scheduler up & running!");
        loop {
            if let Some(chunks) = work_stream.next().await {
                trace!("Received something: {:?}", chunks);
                self.handle_scan_prefix(chunks).await
            } else {
                info!("zmap scheduler shutting down.");
                return Ok(());
            }
        }
    }

    async fn handle_scan_prefix(&self, chunks: Vec<String>) {
        match self.do_scan_prefix(chunks).await {
            Ok(_) => {
                info!("zmap call was successful.");
                // TODO handle results (:
            }
            Err(e) => {
                error!("zmap call failed: {}", e);
                // TODO signal this somehow
            }
        }
    }

    async fn do_scan_prefix(&self, chunks: Vec<String>) -> Result<()> {
        let mut task = SchedulerTask::new(self.zmap_params.clone())?;
        let mut at_least_one_ok = false;
        for chunk in chunks {
            match task.push_work(&chunk) {
                Err(e) => {
                    warn!("Unable to push work {} to task due to {}", chunk, e);
                }
                Ok(_) => at_least_one_ok = true,
            }
        }
        if !at_least_one_ok {
            return Err(anyhow!("None of the work in this chunk could be pushed successfully"));
        }
        let results = task.run().await?;
        info!("Temp results DTO: {:?}", results);
        // TODO forward results to queue handler
        Ok(())
    }
}

struct SchedulerTask {
    store: PrefixSplitProbeStore,
    caller: Caller,
    targets: TargetCollector,
}

type SchedulerWorkItem = String;

impl SchedulerTask {
    fn new(zmap_params: ZmapBaseParams) -> Result<Self> {
        Ok(Self {
            store: probe_store::create(),
            caller: zmap_params.to_caller_assuming_sudo()?,
            targets: TargetCollector::new_default()?,
        })
    }

    fn push_work(&mut self, item: &SchedulerWorkItem) -> Result<()> {
        // TODO permute addresses
        let base_net = item.parse::<Ipv6Net>()
            .with_context(|| "parsing IPv6 prefix")?;
        let samples = prefix_split::process(base_net)
            .with_context(|| "splitting IPv6 prefix")?;
        for sample in samples.iter() {
            self.targets.push_vec(sample.addresses.clone())?;
        }
        self.store.register_request(base_net, samples);
        Ok(())
    }

    // TODO: Pass result in same/different data structure through channel s.t. it can be
    // TODO: sent out
    // TODO: test address: 2a01:4f9:6b:1280::2/126
    // TODO: Don't forget to set rabbitmq credentials in env
    async fn run(mut self) -> Result<Vec<EchoProbeResponse>> {
        let mut response_rx = self.caller.request_responses();
        self.targets.flush()?;
        let zmap_task = tokio::task::spawn_blocking(move || {
            trace!("Now calling zmap");
            self.caller.consume_run(self.targets)
        });
        let mut store = self.store;
        while let Some(record) = response_rx.recv().await {
            trace!("response from zmap: {:?}", record);
            store.register_response(&record);
        }
        response_rx.close(); // ensure nothing else is sent
        zmap_task.await.with_context(|| "during blocking zmap call (await)")??;
        store.fill_missing();
        let models = store.stores.into_iter()
            .map(|it| it.into())
            .collect();
        Ok(models)
    }
}
