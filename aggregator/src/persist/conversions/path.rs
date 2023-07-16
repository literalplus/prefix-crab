use std::io::Write;
use std::net::Ipv6Addr;
use std::str::FromStr;

use anyhow::{bail, Context};
use bitvec::prelude::*;




use diesel::pg::Pg;
use diesel::serialize::{IsNull, Output, ToSql};
use diesel::sql_types::{Integer, Text};

use ipnet::Ipv6Net;

use crate::model::PrefixPath;

use super::super::schema::sql_types::Ltree;


impl ToSql<Ltree, Pg> for PrefixPath
where
    i32: ToSql<Integer, Pg>,
{
    fn to_sql(&self, out: &mut Output<Pg>) -> diesel::serialize::Result {
        // ltree format version; currently version 1 -- 1 byte
        // ref: https://doxygen.postgresql.org/ltree__io_8c_source.html ltree_recv
        out.write_all(&[1])?;
        // string representation of the ltree
        <str as ToSql<Text, Pg>>::to_sql(&self.to_string(), &mut out.reborrow())?;
        Ok(IsNull::No)
    }
}

impl FromStr for PrefixPath {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s_as_bytes = s.as_bytes();
        if s_as_bytes.len() < 3 {
            bail!("Input {:?} is too short, min length is 3", s);
        }
        let root_len_bits = 12u8;
        let mut bits = bitvec![u8, Msb0;];
        let two_hex_bytes = format!("{}0{}", &s[2..3], &s[0..2]); // inverted for byte order
        let root_as_bytes = u16::from_str_radix(&two_hex_bytes, 16).with_context(|| {
            format!(
                "First three characters must be hex: {} / {}",
                two_hex_bytes, s
            )
        })?;
        bits.write_all(&root_as_bytes.to_le_bytes())?;
        bits.truncate(root_len_bits.into()); // discard last four bits, as these aren't part of the prefix

        for dot_idx in (3..s.len()).step_by(2) {
            match s_as_bytes.get(dot_idx) {
                Some(b'.') => {}
                _ => bail!(
                    "Expected a dot at position {} of {:?}, but was: {:?}",
                    dot_idx,
                    s,
                    s_as_bytes.get(dot_idx),
                ),
            }
        }

        let mut work_prefix_len: Option<u8> = None;
        // 4 characters root; (64 - 12) bits left, each has a dot (except the last one)
        let end = 4 + (64 - root_len_bits) * 2 - 1u8;
        for char_idx in (4..end).step_by(2) {
            match s_as_bytes.get(char_idx as usize) {
                Some(b'0') => bits.push(false),
                Some(b'1') => bits.push(true),
                None => {
                    if work_prefix_len.is_none() {
                        work_prefix_len = Some(bits.len() as u8);
                    }
                    // still need to fill up until 64 to get the correct total length
                    bits.push(false);
                }
                _ => bail!(
                    "Expected a bit at position {} of {:?} but was: {:?}",
                    char_idx,
                    s,
                    s_as_bytes.get(char_idx as usize),
                ),
            }
        }
        let prefix_len = match work_prefix_len {
            Some(len) => len,
            None => {
                if s_as_bytes.get(end as usize).is_some() {
                    bail!(
                        "Input was too long; An IPv6 prefix must not be longer than 64 bits to be \
                standards-compliant. Input was: {}",
                        s
                    )
                } else {
                    64u8 // every iteration found something, i.e. full length
                }
            }
        };

        bits.write_all(&[0u8; 8])?;
        let bits_slice = bits.as_raw_slice();
        let addr_buf: [u8; 16] = bits_slice.try_into()?;
        let addr = Ipv6Addr::from(addr_buf);
        let net = Ipv6Net::new(addr, prefix_len)
            .with_context(|| format!("issue with prefix length {} of {:?}", prefix_len, s))?;

        Ok(PrefixPath::from(net))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use anyhow::*;
    use assertor::{assert_that, ResultAssertion};

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
            assert_eq!(Ipv6Net::from(&parsed).to_string(), expected);
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
            &format!("{}.", doc_prefix()),
            &format!("{}.0", doc_prefix()),
        ];
        for input in test_cases {
            // when
            let parsed = PrefixPath::from_str(input);
            // then
            assert_that!(parsed).is_err();
            println!("{:?}", parsed.expect_err("?").to_string());
        }
    }
}
