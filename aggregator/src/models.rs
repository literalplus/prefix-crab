pub mod prefix_tree {
    use std::fmt::{Display, Formatter};
    use std::io::Write;
    use std::str::FromStr;

    use anyhow::Context;
    use bitvec::prelude::*;
    use chrono::NaiveDateTime;
    use diesel;
    use diesel::backend::Backend;
    use diesel::deserialize::{FromSql, FromSqlRow};
    use diesel::expression::AsExpression;
    use diesel::pg::Pg;
    use diesel::prelude::*;
    use diesel::serialize::{IsNull, Output, ToSql};
    use diesel::sql_types::Jsonb;
    use ipnet::Ipv6Net;
    use serde::{Deserialize, Serialize};
    use serde_json;

    use crate::schema::sql_types::Ltree;

    #[derive(diesel_derive_enum::DbEnum, Debug)]
    #[ExistingTypePath = "crate::schema::sql_types::PrefixMergeStatus"]
    pub enum MergeStatus {
        NotMerged,
    }

    #[derive(FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Default)]
    #[diesel(sql_type = Jsonb)]
    pub struct ExtraData {
        // IMPORTANT: Type must stay backwards-compatible with previously-written JSON,
        // i.e. add only optional fields or provide defaults!

        pub ever_responded: bool,
    }

    impl FromSql<Jsonb, Pg> for ExtraData {
        fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            // NOTE: Diesel intentionally doesn't provide this implementation, as it may
            // fail if invalid/unexpected data is stored in the DB... We need to be extra careful.

            let value = <serde_json::Value as FromSql<Jsonb, Pg>>::from_sql(bytes)?;
            Ok(serde_json::from_value(value)?)
        }
    }

    impl ToSql<Jsonb, Pg> for ExtraData {
        fn to_sql(&self, out: &mut Output<Pg>) -> diesel::serialize::Result {
            let value = serde_json::to_value(self)?;
            // We need reborrow() to reduce the lifetime of &mut out; mustn't outlive `value`
            <serde_json::Value as ToSql<Jsonb, Pg>>::to_sql(&value, &mut out.reborrow())
        }
    }

    #[derive(Queryable, Selectable)]
    #[diesel(table_name = crate::schema::prefix_tree)]
    #[diesel(check_for_backend(diesel::pg::Pg))]
    pub struct PrefixTree {
        pub id: i64,
        pub path: PrefixPath,
        pub created: NaiveDateTime,
        pub modified: NaiveDateTime,
        pub is_routed: bool,
        pub merge_status: MergeStatus,
        pub data: ExtraData,
    }

    #[derive(FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Default)]
    #[diesel(sql_type = crate::schema::sql_types::Ltree)]
    pub struct PrefixPath(Ipv6Net);

    impl Into<PrefixPath> for Ipv6Net {
        fn into(self) -> PrefixPath {
            PrefixPath(self)
        }
    }

    impl FromSql<Ltree, Pg> for PrefixPath {
        fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            let as_str = String::from_sql(bytes)?;
            match PrefixPath::from_str(&as_str) {
                Ok(res) => Ok(res),
                Err(e) => Err(e.into()),
            }
        }
    }

    impl ToSql<Ltree, Pg> for PrefixPath {
        fn to_sql(&self, out: &mut Output<Pg>) -> diesel::serialize::Result {
            write!(out, "{}", self)?;
            Ok(IsNull::No)
        }
    }

    impl Display for PrefixPath {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            let octets = self.0.network().octets();
            Self::fmt_root_cidr12(f, octets)?;
            self.fmt_netmask_as_bits(f, octets)
        }
    }

    impl FromStr for PrefixPath {
        type Err = anyhow::Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let mut netmask = String::with_capacity(24);
            netmask.push_str(&s[0..3]);
            let net = Ipv6Net::from_str(&netmask)
                .with_context(|| format!("While converting {} to Ipv6Net", &netmask))?;
            Ok(PrefixPath(net))
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
        use ipnet::Ipv6Net;

        use crate::models::prefix_tree::PrefixPath;

        #[test]
        fn display_longest() -> Result<()> {
            // given
            let ip_net = Ipv6Net::from_str("2001:db8::/64")?;
            let pfx = PrefixPath(ip_net);
            // when
            let formatted = format!("{}", pfx);
            // then
            assert_is_doc_prefix(formatted);
            Ok(())
        }

        fn assert_is_doc_prefix(formatted: String) {
            let zero_nibble = ".0.0.0.0";
            assert_eq!(
                formatted,
                format!(
                    "200.0.0.0.1\
                    {}.1.1.0.1.1.0.1.1.1.0.0.0{}",
                    zero_nibble,
                    zero_nibble.repeat(8)
                )
            );
        }

        #[test]
        fn display_too_long() -> Result<()> {
            // given
            let ip_net = Ipv6Net::from_str("2001:db8::/128")?;
            let pfx = PrefixPath(ip_net);
            // when
            let formatted = format!("{}", pfx);
            // then
            assert_is_doc_prefix(formatted);
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

