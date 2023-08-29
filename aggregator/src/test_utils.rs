use std::collections::HashSet;

pub use ipnet::Ipv6Net;
pub use std::net::Ipv6Addr;

use crate::analyse::{MeasurementTree, HitCount, LhrSource};

pub fn net(input: &str) -> Ipv6Net {
    input.parse().expect(input)
}

pub fn addr(input: &str) -> Ipv6Addr {
    input.parse().expect(input)
}

pub const TREE_BASE_NET: &str = "2001:db8::/32";
pub const TREE_UNRELATED_NET: &str = "2001:db9::/64";
pub const TREE_LEFT_NET: &str = "2001:db8:101::/64";
pub const TREE_RIGHT_NET: &str = "2001:db8:beef::/64";
pub const TREE_RIGHT_NET_ALT: &str = "2001:db8:b00f::/64";

pub fn gen_measurements_complex() -> Vec<MeasurementTree> {
    let left = gen_tree_with_lhr_101(TREE_LEFT_NET, 14);
    let mut first_right = gen_tree_with_lhr_101(TREE_RIGHT_NET, 9);
    gen_add_lhr_beef(&mut first_right, 6);
    let second_right = gen_tree_with_lhr_beef(TREE_RIGHT_NET_ALT, 18);
    vec![left, first_right, second_right]
}

pub const TREE_LHR_101: &str = "2001:db8:101::1";
pub const TREE_LHR_BEEF: &str = "2001:db8:beef::20";

pub fn gen_tree_with_lhr_101(net_str: &str, hits: HitCount) -> MeasurementTree {
    let mut tree = MeasurementTree::empty(net(net_str));
    let sources: HashSet<LhrSource> = vec![LhrSource::TraceResponsive].into_iter().collect();
    tree.add_lhr_no_sum(addr(TREE_LHR_101), sources, hits);
    tree
}

pub fn gen_tree_with_lhr_beef(net_str: &str, hits: HitCount) -> MeasurementTree {
    let mut tree = MeasurementTree::empty(net(net_str));
    gen_add_lhr_beef(&mut tree, hits);
    tree
}

pub fn gen_add_lhr_beef(tree: &mut MeasurementTree, hits: HitCount) {
    let sources: HashSet<LhrSource> = vec![LhrSource::TraceUnresponsive].into_iter().collect();
    tree.add_lhr_no_sum(addr(TREE_LHR_BEEF), sources, hits);
}

