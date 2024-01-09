use anyhow::anyhow;
use diesel::{
    deserialize::FromSql,
    pg::Pg,
    prelude::*,
    sql_types::{Cidr, SmallInt},
};
use ipnet::{IpNet, Ipv6Net};

use crate::analyse::Confidence;

pub struct ConfidenceLoader(Confidence);

impl From<ConfidenceLoader> for Confidence {
    fn from(value: ConfidenceLoader) -> Self {
        value.0
    }
}

impl Queryable<SmallInt, Pg> for ConfidenceLoader
where
    i16: FromSql<SmallInt, Pg>,
{
    type Row = i16;

    fn build(row: i16) -> diesel::deserialize::Result<Self> {
        let val = row
            .clamp(0, Confidence::MAX as i16)
            .try_into()
            .expect("when 0 <= x <= 255, x to fit into u8");
        Ok(ConfidenceLoader(val))
    }
}

pub struct Ipv6NetLoader(Ipv6Net);

impl From<Ipv6NetLoader> for Ipv6Net {
    fn from(value: Ipv6NetLoader) -> Self {
        value.0
    }
}

impl Queryable<Cidr, Pg> for Ipv6NetLoader
where
    IpNet: FromSql<Cidr, Pg>,
{
    type Row = IpNet;

    fn build(row: IpNet) -> diesel::deserialize::Result<Self> {
        match row {
            IpNet::V4(ew) => Err(anyhow!("got IPv4 net {}, we don't do IPv4", ew).into()),
            IpNet::V6(net) => Ok(Ipv6NetLoader(net)),
        }
    }
}
