use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args;
use ipnet::Ipv6Net;
use log::{error, info, trace, warn};
use tokio::sync::mpsc::Receiver;
use tokio_stream::{Stream, StreamExt};
use tokio_stream::wrappers::ReceiverStream;

use crate::probe_store::{self, ProbeStore};
use crate::cmd_logic::ZmapBaseParams;
use crate::prefix_split;
use crate::prefix_split::SubnetSample;
use crate::zmap_call::TargetCollector;

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
                self.handle_scan_prefix_now(chunks).await
            } else {
                info!("zmap scheduler shutting down.");
                return Ok(());
            }
        }
    }

    async fn handle_scan_prefix_now(&self, chunks: Vec<String>) {
        let addrs = chunks.into_iter()
            .flat_map(|pfx| self.split_prefix_to_addresses_or_log(&pfx).into_iter())
            .collect::<Vec<SubnetSample>>();
        // TODO permute addresses
        if addrs.is_empty() {
            warn!("Entire batch failed splitting; skipping.");
            return;
        }
        let call_res = self.spawn_and_await_blocking_caller(addrs).await;
        match call_res {
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

    fn split_prefix_to_addresses_or_log(&self, received_str: &str) -> Vec<SubnetSample> {
        match self.split_prefix_to_addresses(received_str) {
            Ok(addrs) => addrs,
            Err(e) => {
                // TODO signal this somehow
                warn!("Failed to split prefix {} into addresses; skipping: {}", received_str, e);
                vec![]
            }
        }
    }

    fn split_prefix_to_addresses(&self, received_str: &str) -> Result<Vec<SubnetSample>> {
        let base_net = received_str.parse::<Ipv6Net>()
            .with_context(|| "parsing IPv6 prefix")?;
        let splits = prefix_split::process(base_net)
            .with_context(|| "splitting IPv6 prefix")?;
        Ok(splits)
    }

    // TODO: Pass result in same/different data structure through channel s.t. it can be
    // TODO: sent out
    // TODO: test address: 2a01:4f9:6b:1280::2/126
    // TODO: Don't forget to set rabbitmq credentials in env
    async fn spawn_and_await_blocking_caller(&self, samples: Vec<SubnetSample>) -> Result<()> {
        let mut caller = self.zmap_params.to_caller_assuming_sudo()?;
        let mut response_rx = caller.request_responses();
        let addresses = samples.iter()
            .flat_map(|it: &SubnetSample| it.addresses.iter())
            .map(|it| it.clone())
            .collect();
        trace!("Addresses: {:?}", addresses);
        let zmap_task = tokio::task::spawn_blocking(move || {
            let mut targets = TargetCollector::new_default()?;
            targets.push_vec(addresses)?;
            trace!("Now calling zmap");
            caller.consume_run(targets)
        });
        let mut stores = probe_store::create_for(samples);
        while let Some(record) = response_rx.recv().await {
            trace!("response from zmap: {:?}", record);
            stores.register_response(&record);
        }
        response_rx.close(); // ensure nothing else is sent
        zmap_task.await.with_context(|| "during blocking zmap call (await)")??;
        stores.fill_missing();
        info!("Temp output of probe store -> {:?}", stores);
        Ok(())
    }
}
