use std::time::Duration;

use anyhow::{Context, Result};
use clap::Args;
use ipnet::Ipv6Net;
use log::{error, info, trace, warn};
use tokio::sync::mpsc::Receiver;
use tokio_stream::{Stream, StreamExt};
use tokio_stream::wrappers::ReceiverStream;

use crate::cmd_logic::ZmapBaseParams;
use crate::prefix_split;
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

    // TODO: Store addresses in a data structure (-> map)
    // TODO: Record responses with the data structure and detect which ones have none
    // TODO: Pass result in same/different data structure through channel s.t. it can be
    // TODO: sent out
    // TODO: test address: 2a01:4f9:6b:1280::2/126
    // TODO: Don't forget to set rabbitmq credentials in env
    async fn spawn_and_await_blocking_caller(&self, addresses: Vec<String>) -> Result<()> {
        let mut caller = self.zmap_params.to_caller_assuming_sudo()?;
        let mut response_rx = caller.request_responses();
        trace!("Addresses: {:?}", addresses);
        tokio::task::spawn_blocking(move || {
            let mut targets = TargetCollector::new_default()?;
            targets.push_vec(addresses)?;
            trace!("Now calling zmap");
            caller.consume_run(targets)
        }).await.with_context(|| "during blocking zmap call (await)")??;
        response_rx.close(); // ensure nothing else is sent
        while let Some(record) = response_rx.recv().await {
            trace!("response from zmap: {:?}", record);
            // TODO: forward to a result handler (which can also interpret missing responses?)
        }
        // TODO: handle missing responses
        Ok(())
    }
}

mod probe_store {
    use std::collections::{HashMap, HashSet};
    use std::net::Ipv6Addr;

    use crate::prefix_split::SubnetSample;
    use crate::zmap_call::ProbeResponse;

    #[derive(Eq, PartialEq, Hash, Debug)]
    enum DestUnreachKind {
        Other(uint8),
        // 0 = no route, 2 = beyond scope, 7 = source routing error
        AdminProhibited,
        // 1
        AddressUnreachable,
        // 3
        PortUnreachable,
        // 4
        FailedEgressPolicy,
        // 5
        RejectRoute, // 6
    }

    impl DestUnreachKind {
        fn parse(code: u8) -> Self {
            match code {
                1 => Self::AdminProhibited,
                3 => Self::AddressUnreachable,
                4 => Self::PortUnreachable,
                5 => Self::FailedEgressPolicy,
                6 => Self::RejectRoute,
                weird => Self::Other(weird),
            }
        }
    }

    #[derive(Eq, PartialEq, Hash, Debug)]
    enum ResponseKey {
        DestinationUnreachable { kind: DestUnreachKind },
        EchoReply { different_from: Option<Ipv6Addr> },
        NoResponse,
        TimeExceeded { from: Ipv6Addr, sent_ttl: uint8 },
        Other { description: String },
    }

    impl ResponseKey {
        fn from(source: &ProbeResponse) -> Self {
            match source.icmp_type {
                1 => Self::DestinationUnreachable {
                    kind: DestUnreachKind::parse(source.icmp_code),
                },
                3 => Self::TimeExceeded { from: source.source_ip, sent_ttl: source.original_ttl },
                129 => Self::EchoReply {
                    different_from: Some(source.source_ip)
                        .filter(|it| it != source.original_dest_ip),
                },
                other => Self::Other { description: source.classification.to_string() }
            }
        }
    }

    type ResponseCount = u8;

    #[derive(Debug)]
    struct Responses {
        count: ResponseCount,
        intended_targets: Vec<Ipv6Addr>,
    }

    impl Responses {
        fn empty() -> Self {
            return Responses {
                count: 0u8,
                intended_targets: vec![],
            };
        }

        fn add(&mut self, source: &ProbeResponse) {
            self.count = self.count.saturating_add(1u8);
            self.intended_targets.push(source.original_dest_ip);
        }

        fn add_missed(&mut self, addr: Ipv6Addr) {
            self.count = self.count.saturating_add(1u8);
            self.intended_targets.push(addr);
        }
    }

    #[derive(Debug)]
    pub struct ProbeStore {
        // NOTE: Addresses that receive a response are REMOVED from the sample
        sample: SubnetSample,
        responses: HashMap<ResponseKey, Responses>,
    }

    impl ProbeStore {
        pub fn new(sample: SubnetSample) -> ProbeStore {
            return ProbeStore {
                sample,
                responses: HashMap::new(),
            };
        }

        pub fn add(&mut self, response: &ProbeResponse) {
            let key = ResponseKey::from(&response);
            let aggregate = self.entry(key);
            aggregate.add(&response);
            // Using a HashSet here is unlikely to provide a good trade-off, as there
            // will usually only be 16 elements (potentially duplicated for small subnets)
            self.sample.addresses.retain(|el| el != response.original_dest_ip);
        }

        fn entry(&mut self, key: ResponseKey) -> &mut Responses {
            self.responses.entry(key).or_insert(Responses::empty())
        }

        /// Marks all probe targets for which no response has been registered
        /// as [ResponseKey::NoResponse].
        pub fn fill_missing(&mut self) {
            let missing_addrs_iter = self.sample.addresses.drain(..);
            let missing_addrs_uniq = HashSet::from_iter(missing_addrs_iter);
            let no_responses = self.entry(ResponseKey::NoResponse);
            for missing_addr in missing_addrs_uniq {
                no_responses.add_missed(missing_addr);
            }
        }
    }
}
