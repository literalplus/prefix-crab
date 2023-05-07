pub mod probe_request {
    use ipnet::Ipv6Net;

    pub struct EchoProbeRequest {
        pub target_net: Ipv6Net,
    }
}

pub mod echo_response;