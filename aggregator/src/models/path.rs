use std::fmt::{Display, Formatter};
use std::io::Write;
use std::net::Ipv6Addr;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context};
use bitvec::prelude::*;
use diesel;
use diesel::backend::Backend;
use diesel::deserialize::{FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::Pg;
use diesel::serialize::{IsNull, Output, ToSql};
use diesel::sql_types::{Integer, Text};
use ipnet::Ipv6Net;
use serde::{Deserialize, Serialize};

pub use expr::PathExpressionMethods;

use crate::schema::sql_types::Ltree;

#[derive(FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Default, Copy, Clone)]
#[diesel(sql_type = crate::schema::sql_types::Ltree)]
pub struct PrefixPath(Ipv6Net);

pub mod expr {
    use diesel;
    use diesel::Expression;
    use diesel::expression::AsExpression;
    use diesel::pg::Pg;
    use diesel::sql_types::Bool;

    use crate::schema::sql_types::Ltree;

    diesel::infix_operator!(AncestorOrSame, " @> ", Bool, backend: Pg);
    diesel::infix_operator!(DescendantOrSame, " <@ ", Bool, backend: Pg);

    pub trait PathExpressionMethods where Self: AsExpression<Ltree> + Sized {
        // Note: If there are issues with operator precedence (because of missing parentheses),
        // we can copy over Grouped from upstream (which is not public atm sadly)
        // https://github.com/diesel-rs/diesel/blob/13c237473627ea2500c2274e3b0cf8a54187079a/diesel/src/expression/grouped.rs#L8

        fn ancestor_or_same_as<OtherType>(
            self, other: OtherType,
        ) -> AncestorOrSame<Self::Expression, OtherType::Expression>
            where
                OtherType: AsExpression<Ltree>,
        {
            AncestorOrSame::new(self.as_expression(), other.as_expression())
        }

        fn descendant_or_same_as<OtherType>(
            self, other: OtherType,
        ) -> DescendantOrSame<Self::Expression, OtherType::Expression>
            where
                OtherType: AsExpression<Ltree>,
        {
            DescendantOrSame::new(self.as_expression(), other.as_expression())
        }
    }

    impl<T> PathExpressionMethods for T where T: Expression<SqlType=Ltree> {}
}

impl Into<PrefixPath> for Ipv6Net {
    fn into(self) -> PrefixPath {
        PrefixPath(self)
    }
}

impl FromSql<Ltree, Pg> for PrefixPath {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        let buf = bytes.as_bytes();
        if buf[0] != 1u8 {
            return Err(anyhow!("Unexpected ltree version {}, only 1 is supported.", buf[0]).into());
        }
        // Sadly, once we "open up" the RawValue, we cannot re-assemble it since all necessary
        // functions are private / only-for-new-backends in Diesel. So we cannot reuse the
        // existing (zero-copy) String impl but instead need to parse manually.
        let as_str = String::from_utf8(buf[1..].to_vec())
            .with_context(|| "while reading UTF-8 string")?;
        Ok(PrefixPath::from_str(&as_str)?)
    }
}

impl ToSql<Ltree, Pg> for PrefixPath where i32: ToSql<Integer, Pg> {
    fn to_sql(&self, out: &mut Output<Pg>) -> diesel::serialize::Result {
        // ltree format version; currently version 1 -- 1 byte
        // ref: https://doxygen.postgresql.org/ltree__io_8c_source.html ltree_recv
        out.write(&[1])?;
        // string representation of the ltree
        <str as ToSql<Text, Pg>>::to_sql(&self.to_string(), &mut out.reborrow())?;
        Ok(IsNull::No)
    }
}

impl FromStr for PrefixPath {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let root_len_bits = 12u8;
        let mut bits = bitvec![u8, Msb0;];
        let two_hex_bytes = format!("{}0{}", &s[2..3], &s[0..2]); // inverted for byte order
        let root_as_bytes = u16::from_str_radix(&two_hex_bytes, 16)
            .with_context(|| format!(
                "First three characters must be hex: {} / {}", two_hex_bytes, s
            ))?;
        bits.write(&root_as_bytes.to_le_bytes())?;
        bits.truncate(root_len_bits.into()); // discard last four bits, as these aren't part of the prefix

        for dot_idx in (3..s.len()).step_by(2) {
            match s.bytes().nth(dot_idx) {
                Some(b'.') => {}
                _ => bail!(
                    "Expected a dot at position {} of {:?}, but was: {:?}",
                    dot_idx, s, s.bytes().nth(dot_idx),
                ),
            }
        }

        let mut work_prefix_len: Option<u8> = None;
        // 4 characters root; (64 - 12) bits left, each has a dot
        let end = 4 + (64 - root_len_bits) * 2u8;
        for char_idx in (4..end).step_by(2) {
            match s.bytes().nth(char_idx.into()) {
                Some(b'0') => bits.push(false),
                Some(b'1') => bits.push(true),
                None => {
                    if let None = work_prefix_len {
                        work_prefix_len = Some(bits.len() as u8);
                    }
                    // still need to fill up until 64 to get the correct total length
                    bits.push(false);
                }
                _ => bail!(
                    "Expected a bit at position {} of {:?} but was: {:?}",
                    char_idx, s, s.bytes().nth(char_idx.into()),
                ),
            }
        }
        let prefix_len = match work_prefix_len {
            Some(len) => len,
            None => if s.bytes().nth((end + 1) as usize).is_some() {
                bail!(
                "Input was too long; An IPv6 prefix must not be longer than 64 bits to be \
                standards-compliant. Input was: {}", s
            )
            } else {
                64u8 // every iteration found something, i.e. full length
            },
        };

        bits.write_all(&[0u8; 8])?;
        let bits_slice = bits.as_raw_slice();
        let addr_buf: [u8; 16] = bits_slice.try_into()?;
        let addr = Ipv6Addr::from(addr_buf);
        let net = Ipv6Net::new(addr, prefix_len)
            .with_context(|| format!("issue with prefix length {} of {:?}", prefix_len, s))?;

        Ok(PrefixPath(net))
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
        let first_nibble = full_second_byte.chars().nth(0)
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
    use assertor::{assert_that, ResultAssertion};
    use ipnet::Ipv6Net;

    use super::*;

    fn doc_prefix() -> String {
        let zero_nibble: &str = ".0.0.0.0";
        return format!(
            "200.0.0.0.1{}.1.1.0.1.1.0.1.1.1.0.0.0{}",
            zero_nibble,
            zero_nibble.repeat(8),
        );
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

    #[test]
    fn from_str() -> Result<()> {
        // given
        let test_cases = [
            ("204.0.1.1", "2046::/15"),
            ("201", "2010::/12"),
            ("fff.1.1.1.1.1", "ffff:8000::/17"),
            (&doc_prefix(), "2001:db8::/64"),
        ];

        for (input, expected) in test_cases {
            // when
            let parsed = PrefixPath::from_str(input)?;
            // then
            assert_eq!(parsed.0.to_string(), expected);
        }

        Ok(())
    }

    #[test]
    fn from_str_errors_should_err_not_panic() {
        // given
        let test_cases = [
            "",
            "asdf",
            "aaa.7",
            "aaaaa1",
            &format!("{}.0", doc_prefix()),
        ];
        for input in test_cases {
            // when
            let parsed = PrefixPath::from_str(input);
            // then
            assert_that!(parsed).is_err();
            println!("{:?}", parsed);
        }
    }
}
