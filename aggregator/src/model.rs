pub use path::*;

mod path {
    use std::fmt::{Display, Formatter};

    use bitvec::prelude::*;
    
    use diesel::deserialize::FromSqlRow;
    use diesel::expression::AsExpression;
    use ipnet::Ipv6Net;

    #[derive(FromSqlRow, AsExpression, Debug, Default, Copy, Clone)]
    #[diesel(sql_type = crate::persist::schema::sql_types::Ltree)]
    pub struct PrefixPath(Ipv6Net);

    impl From<Ipv6Net> for PrefixPath {
        fn from(value: Ipv6Net) -> Self {
            Self(value)
        }
    }

    impl From<&PrefixPath> for Ipv6Net {
        fn from(value: &PrefixPath) -> Self {
            value.0
        }
    }

    impl Display for PrefixPath {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            let octets = self.0.network().octets();
            Self::fmt_root_cidr12(f, octets)?;
            self.fmt_netmask_as_bits(f, octets)
        }
    }

    impl PrefixPath {
        fn fmt_root_cidr12(f: &mut Formatter, octets: [u8; 16]) -> std::fmt::Result {
            // first 12 bits / 1.5 bytes are root (min IANA allocation), ref:
            // https://www.icann.org/resources/pages/allocation-ipv6-rirs-2012-02-25-en
            // These are represented as numbers, e.g. "200", with the lower half of the
            // second byte cut off, as it is not part of the /12
            write!(f, "{:0>2x}", octets[0])?;
            let full_second_byte = format!("{:0>2x}", octets[1]);
            let first_nibble = full_second_byte
                .chars()
                .next()
                .expect("a byte formatted to hex to have at least one character");
            write!(f, "{}", first_nibble)?;
            Ok(())
        }

        fn fmt_netmask_as_bits(&self, f: &mut Formatter, octets: [u8; 16]) -> std::fmt::Result {
            // next five bytes determine network (as bits)
            // every one gets its own node, s.t. we represent arbitrary net slices
            let bits = octets.view_bits::<Msb0>(); // Msb0 = iterate left-to-right
            let prefix_len = 64.min(self.0.prefix_len().into());
            for bit_idx in 12usize..prefix_len {
                if bits[bit_idx] {
                    f.write_str(".1")?;
                } else {
                    f.write_str(".0")?;
                }
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {

        use std::str::FromStr;

        use anyhow::*;
        use ipnet::Ipv6Net;

        use super::*;

        fn doc_prefix() -> String {
            let zero_nibble: &str = ".0.0.0.0";
            format!(
                "200.0.0.0.1{}.1.1.0.1.1.0.1.1.1.0.0.0{}",
                zero_nibble,
                zero_nibble.repeat(8),
            )
        }

        #[test]
        fn display_longest() -> Result<()> {
            // given
            let ip_net = Ipv6Net::from_str("2001:db8::/64")?;
            let pfx = PrefixPath(ip_net);
            // when
            let formatted = format!("{}", pfx);
            // then
            assert_eq!(formatted, doc_prefix());
            Ok(())
        }

        #[test]
        fn display_too_long() -> Result<()> {
            // given
            let ip_net = Ipv6Net::from_str("2001:db8::/128")?;
            let pfx = PrefixPath(ip_net);
            // when
            let formatted = format!("{}", pfx);
            // then
            assert_eq!(formatted, doc_prefix());
            Ok(())
        }

        #[test]
        fn display_shortest() -> Result<()> {
            // given
            let ip_net = Ipv6Net::from_str("2047::/12")?;
            let pfx = PrefixPath(ip_net);
            // when
            let formatted = format!("{}", pfx);
            // then
            assert_eq!(formatted, "204");
            Ok(())
        }

        #[test]
        fn display_too_short() -> Result<()> {
            // given
            let ip_net = Ipv6Net::from_str("2047::/3")?;
            let pfx = PrefixPath(ip_net);
            // when
            let formatted = format!("{}", pfx);
            // then
            assert_eq!(formatted, "200"); // ipnet cuts off the '4' due to /3
            Ok(())
        }

        #[test]
        fn display_in_between() -> Result<()> {
            // given
            let ip_net = Ipv6Net::from_str("2047:db9::/15")?;
            let pfx = PrefixPath(ip_net);
            // when
            let formatted = format!("{}", pfx);
            // then
            assert_eq!(formatted, "204.0.1.1");
            Ok(())
        }

        #[test]
        fn display_loopback() -> Result<()> {
            // given
            let ip_net = Ipv6Net::from_str("::/15")?;
            let pfx = PrefixPath(ip_net);
            // when
            let formatted = format!("{}", pfx);
            // then
            assert_eq!(formatted, "000.0.0.0");
            Ok(())
        }

        #[test]
        fn display_high() -> Result<()> {
            // given
            let ip_net = Ipv6Net::from_str("ffff:ffff::/17")?;
            let pfx = PrefixPath(ip_net);
            // when
            let formatted = format!("{}", pfx);
            // then
            assert_eq!(formatted, "fff.1.1.1.1.1");
            Ok(())
        }
    }
}
