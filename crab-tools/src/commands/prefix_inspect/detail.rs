use db_model::{
    analyse::{
        subnet::{Subnet, Subnets},
        MeasurementTree,
    },
    persist::{self, dsl::CidrMethods, DieselErrorFixCause},
    prefix_tree::PrefixTree,
};
use diesel::{
    ExpressionMethods, OptionalExtension, PgConnection, QueryDsl, RunQueryDsl, SelectableHelper,
};
use itertools::Itertools;
use prefix_crab::prefix_split::{NetIndex, SplitSubnet};
use std::io::Write;
use std::time::Instant;

use ipnet::Ipv6Net;

pub use component::Detail;

use model::*;

macro_rules! pfxerr {
    ($which:ident) => {
        |source| Error::$which {
            desc: format!("{:?}", source),
        }
    };
}

macro_rules! writepfx {
    (&mut $builder:ident, $($toks:tt)*) => {
        writeln!(&mut $builder.buf, $($toks)*).map_err(pfxerr!(Format))?
    };
}

mod component;
mod model;

pub fn print_prefix(net: Ipv6Net) -> Result {
    let mut buf = PrintedPrefixBuilder::default();
    let mut conn = persist::connect("tools - detail").map_err(pfxerr!(DbConnect))?;

    let tree = load_tree(&mut conn, &net)?;
    writepfx!(
        &mut buf,
        "🌳 Tree data: {} 🍃{:?} 💰{:?} 💪{}%",
        tree.net,
        tree.merge_status,
        tree.priority_class,
        tree.confidence
    );

    let load_start = Instant::now();
    let measurements = load_relevant_measurements(&mut conn, &net)?;

    let all_are_64s = measurements
        .iter()
        .all(|it| it.target_net.prefix_len() == 64);
    let size_desc = if all_are_64s {
        "/64 prefixes"
    } else {
        "merged prefixes"
    };
    writepfx!(
        &mut buf,
        "👀 {} {} probed in this prefix (loaded from DB in {:?})",
        measurements.len(),
        size_desc,
        load_start.elapsed(),
    );

    buf = buf.flush_section()?;

    let nearest_root = load_nearest_root(&mut conn, &tree)?;
    if net.prefix_len() >= 64 {
        let mut fake_subnet: Subnet = SplitSubnet {
            index: NetIndex::try_from(0u8).map_err(pfxerr!(SubnetSplit))?,
            network: net,
        }
        .into();
        for measurement in measurements {
            fake_subnet
                .consume_merge(&measurement)
                .map_err(pfxerr!(SubnetSplit))?;
        }
        buf = print_subnet(buf, &fake_subnet, nearest_root.as_ref())?;
    } else {
        let subnets = Subnets::new(net, measurements).map_err(pfxerr!(SubnetSplit))?;
        for subnet in subnets.iter() {
            buf = print_subnet(buf, subnet, nearest_root.as_ref())?;
        }
    }

    Ok(buf.into())
}

fn print_subnet(
    mut buf: PrintedPrefixBuilder,
    subnet: &Subnet,
    nearest_root: Option<&PrefixTree>,
) -> StdResult<PrintedPrefixBuilder, Error> {
    writepfx!(&mut buf,);
    writepfx!(&mut buf, "▶ Subnet: {}", subnet.subnet.network);

    if subnet.probe_count() == 0 {
        writepfx!(&mut buf, " No probes recorded.");
        return Ok(buf);
    }

    let responsive_percent =
        (subnet.responsive_count() as i64 * 100i64).div_euclid(subnet.probe_count() as i64);
    writepfx!(
        &mut buf,
        " {} Probes, of these: (🔊{} 🔇{}) => {}% responsive",
        subnet.probe_count(),
        subnet.responsive_count(),
        subnet.unresponsive_count(),
        responsive_percent,
    );

    writepfx!(&mut buf, " Last-Hop Routers:");
    for (addr, item) in subnet.iter_lhrs().sorted_by_key(|(addr, _)| *addr) {
        let percent = (item.hit_count as i64 * 100i64).div_euclid(subnet.responsive_count() as i64);
        let is_out_of_prefix = nearest_root
            .map(|root| !root.net.contains(addr))
            .unwrap_or(false);
        let out_of_prefix_marker = if is_out_of_prefix {
            " 🛸"
        } else {
            ""
        };
        writepfx!(
            &mut buf,
            "  🚏 {} - {} hits ({}%){}",
            addr,
            item.hit_count,
            percent,
            out_of_prefix_marker
        );
    }
    writepfx!(&mut buf, " Weirdness:");
    for (typ, item) in subnet.iter_weirds() {
        writepfx!(&mut buf, "  🌪 {:?} - {} hits", typ, item.hit_count);
    }

    buf.flush_section()
}

fn load_tree(conn: &mut PgConnection, target: &Ipv6Net) -> StdResult<PrefixTree, Error> {
    use db_model::schema::prefix_tree::dsl::*;

    prefix_tree
        .filter(net.eq6(target))
        .select(PrefixTree::as_select())
        .first(conn)
        .fix_cause()
        .map_err(pfxerr!(LoadTree))
}

fn load_nearest_root(
    conn: &mut PgConnection,
    leaf: &PrefixTree,
) -> StdResult<Option<PrefixTree>, Error> {
    use db_model::schema::prefix_tree::dsl::*;

    prefix_tree
        .filter(asn.eq(leaf.asn))
        .filter(net.supernet_or_eq6(&leaf.net))
        .select(PrefixTree::as_select())
        .first(conn)
        .optional()
        .fix_cause()
        .map_err(pfxerr!(LoadClosestRoot))
}

fn load_relevant_measurements(
    conn: &mut PgConnection,
    base_net: &Ipv6Net,
) -> StdResult<Vec<MeasurementTree>, Error> {
    use db_model::schema::measurement_tree::dsl::*;

    measurement_tree
        .filter(target_net.subnet_or_eq6(base_net))
        .select(MeasurementTree::as_select())
        .load(conn)
        .fix_cause()
        .map_err(pfxerr!(LoadMeasurements))
}
