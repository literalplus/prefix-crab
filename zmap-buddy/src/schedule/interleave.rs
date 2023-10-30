use std::net::Ipv6Addr;

use prefix_crab::prefix_split::SubnetSample;

pub struct InterleavedTargetsIter {
    samples: Vec<SubnetSample>,
    next_idx: usize,
}

impl InterleavedTargetsIter {
    pub fn new(samples: &[SubnetSample]) -> Self {
        Self {
            samples: samples.to_vec(),
            next_idx: 0,
        }
    }
}

impl Iterator for InterleavedTargetsIter {
    type Item = Ipv6Addr;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.next_idx >= self.samples.len() {
                if self.samples.is_empty() {
                    return None;
                }
                self.next_idx = 0;
            }

            let sample = &mut self.samples[self.next_idx];
            if sample.addresses.is_empty() {
                self.samples.remove(self.next_idx);
                continue;
            }

            self.next_idx += 1;
            return Some(sample.addresses.remove(sample.addresses.len() - 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv6Addr;

    use assertor::{assert_that, OptionAssertion};
    use prefix_crab::prefix_split::SubnetSample;

    use super::InterleavedTargetsIter;

    #[test]
    fn empty() {
        // given
        let mut instance = InterleavedTargetsIter::new(&[]);

        // when, then
        assert_that!(instance.next()).is_none();
    }

    #[test]
    fn empty_inner() {
        // given
        let mut instance = InterleavedTargetsIter::new(&[addrs(&[])]);

        // when, then
        assert_that!(instance.next()).is_none();
    }

    fn addrs(addrs: &[Ipv6Addr]) -> SubnetSample {
        let sample = SubnetSample {
            index: 0u8.try_into().unwrap(),
            network: "2001:db8::/32".parse().unwrap(),
            addresses: addrs.to_vec(),
        };
        sample
    }

    #[test]
    fn two_empty_inner() {
        // given
        let mut instance = InterleavedTargetsIter::new(&[addrs(&[]), addrs(&[])]);

        // when, then
        assert_that!(instance.next()).is_none();
    }

    #[test]
    fn single() {
        // given
        let first = addr("2001:db8::1");
        let second = addr("2001:db8::2");
        let sample = addrs(&[first, second]);
        let mut instance = InterleavedTargetsIter::new(&[sample]);

        // when, then
        assert_that!(instance.next()).has_value(second);
        assert_that!(instance.next()).has_value(first);
        assert_that!(instance.next()).is_none();
    }

    pub fn addr(input: &str) -> Ipv6Addr {
        input.parse().expect(input)
    }

    #[test]
    fn dual() {
        // given
        let first_left = addr("2001:db8::1");
        let second_left = addr("2001:db8::2");
        let first_right = addr("2001:db8::3");
        let second_right = addr("2001:db8::4");
        let third_right = addr("2001:db8::5");
        let mut instance = InterleavedTargetsIter::new(&[
            addrs(&[first_left, second_left]),
            addrs(&[first_right, second_right, third_right]),
        ]);

        // when, then
        assert_that!(instance.next()).has_value(second_left);
        assert_that!(instance.next()).has_value(third_right);
        assert_that!(instance.next()).has_value(first_left);
        assert_that!(instance.next()).has_value(second_right);
        assert_that!(instance.next()).has_value(first_right);
        assert_that!(instance.next()).is_none();
    }

    #[test]
    fn dual_first_longer() {
        // given
        let first_left = addr("2001:db8::1");
        let second_left = addr("2001:db8::2");
        let third_left = addr("2001:db8::18");
        let first_right = addr("2001:db8::3");
        let second_right = addr("2001:db8::4");
        let mut instance = InterleavedTargetsIter::new(&[
            addrs(&[first_left, second_left, third_left]),
            addrs(&[first_right, second_right]),
        ]);

        // when, then
        assert_that!(instance.next()).has_value(third_left);
        assert_that!(instance.next()).has_value(second_right);
        assert_that!(instance.next()).has_value(second_left);
        assert_that!(instance.next()).has_value(first_right);
        assert_that!(instance.next()).has_value(first_left);
        assert_that!(instance.next()).is_none();
    }
}