use nohash_hasher::IntMap;
use std::{
    collections::hash_map,
    iter::{FusedIterator, Iterator},
    net::Ipv6Addr,
    ops::Index,
    ops::IndexMut,
};

use ipnet::Ipv6Net;

const PREFIX_LEN: u8 = 64;

/// A map keyed by /64 networks. Currently assumes this exact size, but may
/// be expanded to less-specific (but not more-specific) masks in the future.
#[derive(Debug)]
pub struct Net64Map<V> {
    per_net: IntMap<u64, V>,
}

// derive macro falsely results in requiring that V: Default
impl<V> Default for Net64Map<V> {
    fn default() -> Self {
        Self { per_net: Default::default() }
    }
}

fn to_key(net: &Ipv6Net) -> u64 {
    assert!(
        net.prefix_len() == PREFIX_LEN,
        "to_key({}) can only accept /64 networks",
        net
    );
    addr_to_key(&net.network())
}

fn addr_to_key(addr: &Ipv6Addr) -> u64 {
    let raw = u128::from(*addr);
    (raw >> PREFIX_LEN) as u64
}

fn addr_to_net(addr: Ipv6Addr) -> Ipv6Net {
    Ipv6Net::new(addr, PREFIX_LEN).expect("/64 to be a valid prefix length")
}

fn net_from_key(key: &u64) -> Ipv6Net {
    let expanded = *key as u128;
    let shifted = expanded
        .checked_shl(PREFIX_LEN as u32)
        .expect("Failed to shift-left u64 by 64 (which should always work)");
    addr_to_net(Ipv6Addr::from(shifted))
}

pub struct Iter<'a, V> {
    delegate: hash_map::Iter<'a, u64, V>,
}

impl<V> Net64Map<V> {
    pub fn iter(&self) -> Iter<'_, V> {
        Iter {
            delegate: self.per_net.iter(),
        }
    }
}

impl<'a, V> Iterator for Iter<'a, V> {
    type Item = (Ipv6Net, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let delegated = self.delegate.next();
        if delegated.is_none() {
            return None;
        }
        let (raw_mask, value) = delegated.unwrap();
        Some((net_from_key(raw_mask), value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.delegate.size_hint()
    }
}

impl<V> FusedIterator for Iter<'_, V> {}

impl<V> Index<&Ipv6Net> for Net64Map<V> {
    type Output = V;

    fn index(&self, idx: &Ipv6Net) -> &Self::Output {
        self.per_net.index(&to_key(idx))
    }
}

impl<V> Index<&Ipv6Addr> for Net64Map<V> {
    type Output = V;

    fn index(&self, idx: &Ipv6Addr) -> &Self::Output {
        self.per_net.index(&addr_to_key(idx))
    }
}

impl<V> Net64Map<V> {
    fn entry_by_net(&mut self, net: &Ipv6Net) -> hash_map::Entry<'_, u64, V> {
        self.per_net.entry(to_key(net))
    }

    pub fn entry_by_net_or(&mut self, net: &Ipv6Net, new_fn: fn(Ipv6Net) -> V) -> &mut V {
        self.entry_by_net(net)
            .or_insert_with(|| new_fn(*net))
    }

    fn entry_by_addr(&mut self, addr: &Ipv6Addr) -> hash_map::Entry<'_, u64, V> {
        self.per_net.entry(addr_to_key(addr))
    }

    pub fn entry_by_addr_or(&mut self, addr: &Ipv6Addr, new_fn: fn(Ipv6Net) -> V) -> &mut V {
        self.entry_by_net_or(&addr_to_net(*addr), new_fn)
    }
}

impl<V> IndexMut<&Ipv6Net> for Net64Map<V>
where
    V: Default,
{
    fn index_mut(&mut self, idx: &Ipv6Net) -> &mut Self::Output {
        self.entry_by_net(idx).or_default()
    }
}

impl<V> IndexMut<&Ipv6Addr> for Net64Map<V>
where
    V: Default,
{
    fn index_mut(&mut self, idx: &Ipv6Addr) -> &mut Self::Output {
        self.entry_by_addr(idx).or_default()
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use ipnet::Ipv6Net;
    use std::{net::Ipv6Addr, str::FromStr};

    use super::{addr_to_key, to_key, Net64Map};

    #[test]
    fn test_put_and_get() -> Result<()> {
        // given
        let mut store = Net64Map::default();
        let addr = Ipv6Addr::from_str("2001:db8::56")?;
        let net = Ipv6Net::from_str("2001:db8::/64")?;
        // when
        store[&addr] = 42;
        // then
        assert_eq!(store[&net], 42);
        Ok(())
    }

    #[test]
    fn test_iter_correct_net() -> Result<()> {
        // given
        let mut store = Net64Map::default();
        let addr = Ipv6Addr::from_str("2001:db8::56")?;
        let net = Ipv6Net::from_str("2001:db8::/64")?;
        store[&addr] = 42;
        // when
        let mut iter = store.iter();
        // then
        assert_eq!(iter.next(), Some((net, &42)));
        assert_eq!(iter.next(), None);
        Ok(())
    }

    #[test]
    fn test_addr_to_net() -> Result<()> {
        // given
        let addr = Ipv6Addr::from_str("2001:db8::56")?;
        let net_key = to_key(&Ipv6Net::from_str("2001:db8::/64")?);
        // when
        let addr_key = addr_to_key(&addr);
        // then
        assert_eq!(addr_key, net_key);
        Ok(())
    }

    #[test]
    fn test_key_conversion() -> Result<()> {
        // given
        let expected = 0x2001db8000000u64;
        let net = Ipv6Net::from_str("2001:db8::865/64")?;
        // when
        let net_key = to_key(&net);
        // then
        assert_eq!(net_key, expected);
        Ok(())
    }
}
