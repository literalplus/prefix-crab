use std::time::Duration;

use anyhow::{Context, Result};
use ipnet::Ipv6Net;
use log::{error, info, trace, warn, debug};
use tokio::select;
use tokio::sync::mpsc::Receiver;
use tokio_stream::{Stream, StreamExt};
use tokio_stream::wrappers::{ReceiverStream, UnboundedReceiverStream};

use crate::cmd_logic::ZmapBaseParams;
use crate::prefix_split;
use crate::zmap_call::{ProbeResponse, TargetCollector};

struct Scheduler {
    zmap_params: ZmapBaseParams,
}

pub async fn start(
    work_rx: Receiver<String>,
    zmap_params: ZmapBaseParams,
) -> Result<()> {
    let work_stream = ReceiverStream::new(work_rx)
        .chunks_timeout(16, Duration::from_secs(60));
    Scheduler { zmap_params }.run(work_stream).await
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
            .collect::<Vec<String>>();
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

    fn split_prefix_to_addresses_or_log(&self, received_str: &str) -> Vec<String> {
        match self.split_prefix_to_addresses(received_str) {
            Ok(addrs) => addrs,
            Err(e) => {
                // TODO signal this somehow
                warn!("Failed to split prefix {} into addresses; skipping: {}", received_str, e);
                vec![]
            }
        }
    }

    fn split_prefix_to_addresses(&self, received_str: &str) -> Result<Vec<String>> {
        let base_net = received_str.parse::<Ipv6Net>()
            .with_context(|| "parsing IPv6 prefix")?;
        let splits = prefix_split::process(base_net)
            .with_context(|| "splitting IPv6 prefix")?;
        Ok(splits.into_iter().flatten().map(|addr| addr.to_string()).collect())
    }

    async fn spawn_and_await_blocking_caller(&self, addresses: Vec<String>) -> Result<()> {
        let mut caller = self.zmap_params.to_caller_assuming_sudo()?;
        let response_stream = UnboundedReceiverStream::new(caller.request_responses())
            .fuse();
        trace!("Addresses: {:?}", addresses);
        let mut join_handle = tokio::task::spawn_blocking(move || {
            let mut targets = TargetCollector::new_default()?;
            targets.push_vec(addresses)?;
            trace!("Now calling zmap");
            caller.consume_run(targets)
        });
        tokio::pin!(response_stream);
        loop {
            select! {
                biased; // handle all responses before exiting TODO: is this really safe?
                untyped = response_stream.next() => {
                    let res: Option<ProbeResponse> = untyped;
                    debug!("Received from zmap: {:?}", res);
                },
                untyped = &mut join_handle => {
                    let res: Result<()> = untyped?.with_context(|| "failed to join zmap");
                    if let Err(e) = res {
                        error!("zmap call failed: {}", e);
                        return Err(e).with_context(|| "during zmap call");
                    } else {
                        break; // important; otherwise panic
                    }
                }
            }
        }
        // TODO: handle missing responses
        Ok(())
    }
}
