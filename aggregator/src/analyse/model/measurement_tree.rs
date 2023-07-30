use std::{
    collections::{HashMap, HashSet},
    net::Ipv6Addr, ops::IndexMut,
};

use chrono::{NaiveDateTime, Utc};
use diesel::{prelude::*, sql_types::Jsonb, AsExpression, FromSqlRow};
use ipnet::{IpNet, Ipv6Net};
use serde::{Deserialize, Serialize};

use crate::analyse::map64::Net64Map;

use super::HitCount;

#[derive(Queryable, Selectable, Identifiable, Debug, Clone)]
#[diesel(table_name = crate::schema::measurement_tree)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(target_net))]
pub struct MeasurementTree {
    pub target_net: IpNet,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub hit_count: i32,
    pub miss_count: i32,
    pub last_hop_routers: LhrData,
    pub weirdness: WeirdData,
}

impl MeasurementTree {
    pub fn empty(target_net: Ipv6Net) -> Self {
        Self {
            target_net: IpNet::V6(target_net),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
            hit_count: 0,
            miss_count: 0,
            last_hop_routers: LhrData::default(),
            weirdness: WeirdData::default(),
        }
    }
}

impl IndexMut<&Ipv6Net> for Net64Map<MeasurementTree> {
    fn index_mut(&mut self, idx: &Ipv6Net) -> &mut Self::Output {
        self.entry_by_net_or(idx, MeasurementTree::empty)
    }
}

impl IndexMut<&Ipv6Addr> for Net64Map<MeasurementTree> {
    fn index_mut(&mut self, idx: &Ipv6Addr) -> &mut Self::Output {
        self.entry_by_addr_or(idx, MeasurementTree::empty)
    }
}

/// Last Hop Router in the context of a [MeasurementTree] node.
#[derive(FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Default, Clone)]
#[diesel(sql_type = Jsonb)]
pub struct LhrData {
    // IMPORTANT: Type must stay backwards-compatible with previously-written JSON,
    // i.e. add only optional fields or provide defaults!
    pub items: HashMap<Ipv6Addr, LhrItem>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LhrSource {
    TraceUnresponsive,
    TraceResponsive,
    DestUnreachProhibit, // admin-prohibit, failed-egress
    DestUnreachAddrPort, // addr/port unreach
    DestUnreachReject,   // reject-route
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LhrItem {
    pub sources: HashSet<LhrSource>,
    pub hit_count: HitCount,
}

crate::persist::configure_jsonb_serde!(LhrData);

#[derive(FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Default, Clone)]
#[diesel(sql_type = Jsonb)]
pub struct WeirdData {
    // IMPORTANT: Type must stay backwards-compatible with previously-written JSON,
    // i.e. add only optional fields or provide defaults!
    pub items: HashMap<Ipv6Addr, HitCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WeirdItem {
    pub descriptions: HashSet<String>,
    pub hit_count: HitCount,
}

crate::persist::configure_jsonb_serde!(WeirdData);

// Dumb joke do not use
// FIXME do we need this ... can we just use Net64Map directly ... depends on how we actually insert & dedup into the DB
pub mod forest {
    use crate::analyse::map64::Net64Map;
 
    use super::MeasurementTree;

    #[derive(Default)]
    pub struct MeasurementForest {
        trees: Net64Map<MeasurementTree>,
    }
}
