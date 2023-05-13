pub enum ProbeType {
    /// Scan a prefix using ICMP Echo Requests by splitting it into sub-prefixes
    /// and selecting random addresses to probe.
    EchoScanPrefix,

    /// Trace a set of addresses in a prefix to determine the Last-Hop-Router.
    FollowUpTrace,
}

pub mod probe_request {
    use ipnet::Ipv6Net;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct EchoProbeRequest {
        pub target_net: Ipv6Net,
    }
}

pub mod echo_response;
