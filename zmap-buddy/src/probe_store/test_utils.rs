use std::net::Ipv6Addr;

use anyhow::*;
use ipnet::Ipv6Net;

use prefix_crab::prefix_split::*;
use crate::probe_store::model::ResponseKey;
use crate::probe_store::subnet::SubnetStore;
use crate::schedule::ProbeResponse;

pub fn gen_any_sample() -> Result<SubnetSample> {
    gen_sample("2001:db8::/32")
}

pub fn gen_sample(ipv6_net_str: &str) -> Result<SubnetSample> {
    let net = ipv6_net_str.parse::<Ipv6Net>()?;
    let prefix = split(net)?.to_samples(16)
        .into_iter().next().ok_or(anyhow!("no addrs in prefix"))?;
    Ok(prefix)
}

pub fn gen_timxceed(ip_addr: Ipv6Addr) -> ProbeResponse {
    ProbeResponse {
        source_ip: ip_addr,
        original_dest_ip: ip_addr,
        classification: "aa".to_string(),
        original_ttl: 45,
        icmp_type: 3, // time-exceeded
        icmp_code: 0,
    }
}

pub fn then_timxceed_registered(store: &mut SubnetStore, ip_addr: Ipv6Addr) {
    let key = ResponseKey::TimeExceeded {
        from: ip_addr,
        sent_ttl: 45,
    };
    let res = &store[key];
    assert_eq!(res.len(), 1);
    assert_eq!(res.intended_targets, vec![ip_addr]);
    assert!(!store.is_waiting_for_response(ip_addr));
}

pub fn then_no_timxceed_registered(store: &mut SubnetStore) {
    for stored in store.iter() {
        let (key, responses) = stored;
        match *key {
            ResponseKey::TimeExceeded { from: _, sent_ttl: _ } => assert_eq!(responses.len(), 0),
            _ => {}
        }
    }
}
