use anyhow::*;
use ipnet::Ipv6Net;

pub use sample::{IntoSubnetSamples, SubnetSample};
pub use split::{PrefixSplit, SplitSubnet};

pub const SAMPLES_PER_SUBNET: u16 = 16;
pub const PREFIX_BITS_PER_SPLIT: u8 = 2;
pub const SUBNETS_PER_SPLIT: u8 = 2u8.pow(PREFIX_BITS_PER_SPLIT as u32);
pub const MAX_PREFIX_LEN: u8 = 64;

/**
Splits given network into [SUBNETS_PER_SPLIT] subnets, by increasing the
prefix length by [PREFIX_BITS_PER_SPLIT] up to a maximum of
[MAX_PREFIX_LEN].
 */
pub fn split(base_net: Ipv6Net) -> Result<PrefixSplit> {
    split::process(base_net)
}

mod split {
    use std::iter::IntoIterator;

    use std::ops::Index;

    use anyhow::*;
    use ipnet::Ipv6Net;

    use super::{MAX_PREFIX_LEN, PREFIX_BITS_PER_SPLIT, SUBNETS_PER_SPLIT};

    pub fn process(base_net: Ipv6Net) -> Result<PrefixSplit> {
        let subnet_prefix_len = subnet_prefix_len_for(base_net).context("Base net too small")?;
        let subnets_vec: Vec<Ipv6Net> = base_net
            .subnets(subnet_prefix_len)
            .with_context(|| {
                format!(
                    "Cannot split target prefix {} into /{} subnets",
                    base_net, subnet_prefix_len
                )
            })?
            .collect();
        let subnets: SplitSubnetsRaw = subnets_vec
            .try_into()
            .expect("split with n bits to yield 2^n subnets");
        Ok(PrefixSplit::new(base_net, subnet_prefix_len, subnets))
    }

    fn subnet_prefix_len_for(base_net: Ipv6Net) -> Result<u8> {
        let len = base_net.prefix_len() + PREFIX_BITS_PER_SPLIT;
        if len > MAX_PREFIX_LEN {
            bail!(
                "Cannot further split this prefix {} max split prefix len is {}",
                base_net,
                MAX_PREFIX_LEN
            );
        }
        Ok(len)
    }

    type SplitSubnetsRaw = [Ipv6Net; SUBNETS_PER_SPLIT as usize];
    type SplitSubnets = [SplitSubnet; SUBNETS_PER_SPLIT as usize];

    #[derive(Debug)]
    #[readonly::make]
    pub struct PrefixSplit {
        pub base_net: Ipv6Net,
        pub subnet_prefix_len: u8,
        subnets: SplitSubnets,
    }

    impl PrefixSplit {
        fn new(base_net: Ipv6Net, subnet_prefix_len: u8, subnets_raw: SplitSubnetsRaw) -> Self {
            let mut next_index = 0;
            let subnets = subnets_raw.map(|network| {
                next_index += 1;
                SplitSubnet {
                    index: next_index - 1,
                    network,
                }
            });
            Self {
                base_net,
                subnet_prefix_len,
                subnets,
            }
        }
    }

    impl<'a> IntoIterator for &'a PrefixSplit {
        type Item = &'a SplitSubnet;
        type IntoIter = std::slice::Iter<'a, SplitSubnet>;

        fn into_iter(self) -> Self::IntoIter {
            self.subnets.iter()
        }
    }

    impl<'a> PrefixSplit {
        fn iter(&'a self) -> <&'a Self as IntoIterator>::IntoIter {
            self.into_iter()
        }
    }

    impl<'a> Index<u8> for &'a PrefixSplit {
        type Output = SplitSubnet;

        fn index(&self, index: u8) -> &Self::Output {
            (&self.subnets).index(index as usize)
        }
    }

    #[derive(Debug)]
    pub struct SplitSubnet {
        pub index: u8,
        pub network: Ipv6Net,
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use assertor::*;

        #[test]
        fn prefix_len_normal_case() -> Result<()> {
            // given
            let net = "2001:db8::/32".parse::<Ipv6Net>()?;

            // when
            let len = subnet_prefix_len_for(net);

            // then
            assert_that!(len).is_ok();
            assert_that!(len.unwrap()).is_equal_to(32 + PREFIX_BITS_PER_SPLIT);
            Ok(())
        }

        #[test]
        fn prefix_len_boundary() -> Result<()> {
            // given
            let net = "2001:db8::/62".parse::<Ipv6Net>()?;

            // when
            let len = subnet_prefix_len_for(net);

            // then
            assert_that!(len).is_ok();
            assert_that!(len.unwrap()).is_equal_to(MAX_PREFIX_LEN);
            Ok(())
        }

        #[test]
        fn prefix_len_max() -> Result<()> {
            // given
            let net = "2001:db8::/64".parse::<Ipv6Net>()?;

            // when
            let len = subnet_prefix_len_for(net);

            // then
            assert_that!(len).is_err();
            Ok(())
        }

        #[test]
        fn subnets_at_boundaries() -> Result<()> {
            // given
            let net = "2001:db8::/32".parse::<Ipv6Net>()?;
            let expected_subnets: Vec<Ipv6Net> = vec![
                "2001:db8::/34",
                "2001:db8:4000::/34",
                "2001:db8:8000::/34",
                "2001:db8:c000::/34",
            ]
            .iter()
            .map(|s| s.parse().unwrap())
            .collect();

            // when
            let result = process(net)?;

            // then
            let actual_subnets: Vec<Ipv6Net> = result.iter().map(|r| r.network).collect();
            assert_eq!(actual_subnets, expected_subnets);
            Ok(())
        }

        #[test]
        fn split_object_indices() -> Result<()> {
            // given
            let net = "2001:db8::/32".parse::<Ipv6Net>()?;

            // when
            let result = process(net)?;

            // then
            for (index, subnet) in result.iter().enumerate() {
                assert_that!(subnet.index).is_equal_to(&(index as u8));
            }
            Ok(())
        }
    }
}

mod sample {
    use std::{net::Ipv6Addr, ops::Range};

    use ipnet::{IpAdd, Ipv6Net};
    use rand::distributions::{Distribution, Uniform};

    use super::PrefixSplit;

    #[derive(Debug, Clone)]
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

    pub trait IntoSubnetSamples {
        fn into_samples(self, hosts_per_sample: u16) -> Vec<SubnetSample>;
    }

    impl IntoSubnetSamples for PrefixSplit {
        fn into_samples(self, hosts_per_sample: u16) -> Vec<SubnetSample> {
            let distribution = Uniform::from(determine_host_range(&self));
            self.into_iter()
                .to_owned()
                .map(|subnet| into_sample(subnet.network, distribution, hosts_per_sample))
                .collect()
        }
    }

    fn determine_host_range(prefix_split: &PrefixSplit) -> Range<u128> {
        let free_bits = prefix_split.base_net.max_prefix_len() - prefix_split.subnet_prefix_len;
        0_u128..(2_u128.pow(free_bits as u32))
    }

    fn into_sample(
        subnet: Ipv6Net,
        distribution: Uniform<u128>,
        hosts_per_sample: u16,
    ) -> SubnetSample {
        let base_addr = subnet.network();
        let mut rng = rand::thread_rng();
        let addresses = (0..hosts_per_sample)
            .map(|_| base_addr.saturating_add(distribution.sample(&mut rng)))
            .collect();
        SubnetSample { subnet, addresses }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use anyhow::*;

        #[test]
        fn addresses_in_subnet() -> Result<()> {
            // given
            let net = "2001:db8::/32".parse::<Ipv6Net>()?;
            let first_subnet = "2001:db8::/34".parse::<Ipv6Net>()?;
            let split = super::super::split(net)?;
            // when
            let result = &split.into_samples(512)[0];

            // then
            for address in &result.addresses {
                assert!(first_subnet.contains(address));
            }
            Ok(())
        }
    }
}
