use std::cmp::min;
use std::net::Ipv6Addr;
use std::iter::IntoIterator;

use anyhow::{Context, Result};
use ipnet::{IpAdd, Ipv6Net};
use rand::distributions::{Distribution, Uniform};

#[derive(Debug)]
pub struct SubnetSample {
    pub subnet: Ipv6Net,
    pub addresses: Vec<Ipv6Addr>,
}

impl IntoIterator for SubnetSample {
    type Item = Ipv6Addr;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.addresses.into_iter()
    }
}

pub fn process(base_net: Ipv6Net) -> Result<Vec<SubnetSample>> {
    Splitter::new(base_net).process()
}

struct Splitter {
    base_net: Ipv6Net,
    subnet_prefix_len: u8,
    samples_per_subnet: u32,
    distribution: Uniform<u128>,
}

impl Splitter {
    fn new(base_net: Ipv6Net) -> Self {
        let subnet_prefix_len = min(
            base_net.prefix_len() + 2, base_net.max_prefix_len(),
        );
        let free_bits = base_net.max_prefix_len() - subnet_prefix_len;
        let host_range = 0_u128..(2_u128.pow(free_bits as u32));
        return Splitter {
            base_net: base_net.trunc(),
            subnet_prefix_len,
            samples_per_subnet: 16,
            distribution: Uniform::from(host_range),
        };
    }

    fn process(self) -> Result<Vec<SubnetSample>> {
        let subnets = self.base_net.subnets(self.subnet_prefix_len)
            .with_context(|| format!(
                "Cannot split target prefix {} into /{} subnets",
                self.base_net, self.subnet_prefix_len,
            ))?;
        Ok(
            subnets.map(|it| self.create_subnet_sample(it)).collect()
        )
    }

    fn create_subnet_sample(&self, subnet: Ipv6Net) -> SubnetSample {
        let base_addr = subnet.addr();
        let mut rng = rand::thread_rng();
        let addresses = (0..self.samples_per_subnet)
            .map(|_| base_addr.saturating_add(self.distribution.sample(&mut rng)))
            .collect();
        SubnetSample {
            subnet,
            addresses,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameters_selected_correctly() -> Result<()> {
        // given
        let net = "2001:db8::/32".parse::<Ipv6Net>()?;

        // when
        let instance = Splitter::new(net);

        // then
        assert_eq!(instance.base_net, net);
        assert_eq!(instance.subnet_prefix_len, 32 + 2);
        assert_eq!(instance.samples_per_subnet, 16);
        Ok(())
    }

    #[test]
    fn subnets_at_boundaries() -> Result<()> {
        // given
        let net = "2001:db8::/32".parse::<Ipv6Net>()?;
        let expected_subnets: Vec<Ipv6Net> = vec![
            "2001:db8::/34", "2001:db8:4000::/34", "2001:db8:8000::/34", "2001:db8:c000::/34",
        ].iter().map(|s| s.parse().unwrap()).collect();

        // when
        let result = Splitter::new(net).process()?;

        // then
        let actual_subnets: Vec<Ipv6Net> = result.iter().map(|r| r.subnet).collect();
        assert_eq!(actual_subnets, expected_subnets);
        Ok(())
    }

    #[test]
    fn addresses_in_subnet() -> Result<()> {
        // given
        let net = "2001:db8::/32".parse::<Ipv6Net>()?;
        let first_subnet = "2001:db8::/34".parse::<Ipv6Net>()?;

        // when
        let result = &Splitter::new(net).process()?[0];

        // then
        for address in &result.addresses {
            assert!(first_subnet.contains(address));
        }
        Ok(())
    }
}
