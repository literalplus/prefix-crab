use anyhow::{Context, Result};
use ipnet::Ipv6Net;
use log::{debug, error, info, trace};
use tokio::sync::mpsc::Receiver;

use crate::cmd_logic::ZmapBaseParams;
use crate::prefix_split;
use crate::zmap_call::TargetCollector;

struct QueueHandler {
    work_receiver: Receiver<String>,
    zmap_params: ZmapBaseParams,
}

pub async fn start_handler(
    work_receiver: Receiver<String>,
    zmap_params: ZmapBaseParams,
) -> Result<()> {
    QueueHandler { work_receiver, zmap_params }.run().await
}

impl QueueHandler {
    async fn run(&mut self) -> Result<()> {
        let params = self.zmap_params.clone();
        tokio::task::spawn_blocking(move || {
            params.to_caller_verifying_sudo()?.verify_sudo_access()?;
            Ok::<(), anyhow::Error>(())
        }).await.with_context(|| "sudo access check failed")??;
        loop {
            if let Some(received) = self.work_receiver.recv().await {
                trace!("Received something: {}", received);
                self.handle_scan_prefix_now(received).await
            } else {
                debug!("Queue handler shutting down.");
                return Ok(());
            }
        }
    }

    async fn handle_scan_prefix_now(&self, received_str: String) {
        let addr_res = self.split_prefix_to_addresses(received_str);
        let call_res = match addr_res {
            Ok(addrs) => self.spawn_and_await_blocking_caller(addrs).await,
            Err(e) => Err(e),
        };
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

    fn split_prefix_to_addresses(&self, received_str: String) -> Result<Vec<String>> {
        let base_net = received_str.parse::<Ipv6Net>()
            .with_context(|| "parsing IPv6 prefix")?;
        let splits = prefix_split::process(base_net)
            .with_context(|| "splitting IPv6 prefix")?;
        // TODO permute addresses, aggregate multiple prefixes up to a batch size
        Ok(splits.into_iter().flatten().map(|addr| addr.to_string()).collect())
    }

    async fn spawn_and_await_blocking_caller(&self, addresses: Vec<String>) -> Result<()> {
        let caller = self.zmap_params.to_caller_assuming_sudo()?;
        trace!("Addresses: {:?}", addresses);
        tokio::task::spawn_blocking(move || {
            let mut targets = TargetCollector::new_default()?;
            targets.push_vec(addresses)?;
            trace!("Now calling zmap");
            caller.consume_run(targets)
        }).await.with_context(|| "during blocking zmap call (await)")?
    }
}
