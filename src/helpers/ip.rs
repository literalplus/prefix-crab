use std::net::{IpAddr, Ipv6Addr};

use ipnet::{IpNet, Ipv6Net};

pub trait ExpectV6 {
    type V6Type;

    /// Panics if this struct is not an IPv6 address or net.
    fn expect_v6(self) -> Self::V6Type;
}

impl<'a> ExpectV6 for &'a IpNet {
    type V6Type = &'a Ipv6Net;

    fn expect_v6(self) -> Self::V6Type {
        if let IpNet::V6(it) = self {
            it
        } else {
            panic!("Expected {:?} to be an IPv6 network, but was not", self);
        }
    }
}

impl ExpectV6 for IpNet {
    type V6Type = Ipv6Net;

    fn expect_v6(self) -> Self::V6Type {
        *(&self).expect_v6()
    }
}

impl<'a> ExpectV6 for &'a IpAddr {
    type V6Type = &'a Ipv6Addr;

    fn expect_v6(self) -> Self::V6Type {
        if let IpAddr::V6(it) = self {
            it
        } else {
            panic!("Expected {:?} to be an IPv6 network, but was not", self);
        }
    }
}

impl ExpectV6 for IpAddr {
    type V6Type = Ipv6Addr;

    fn expect_v6(self) -> Self::V6Type {
        *(&self).expect_v6()
    }
}
