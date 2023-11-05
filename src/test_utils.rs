use std::net::Ipv6Addr;

use ipnet::Ipv6Net;


pub fn net(input: &str) -> Ipv6Net {
    input.parse().expect(input)
}

pub fn addr(input: &str) -> Ipv6Addr {
    input.parse().expect(input)
}
