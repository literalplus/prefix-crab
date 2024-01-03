use db_model::{
    analyse::{subnet::Subnets, MeasurementTree},
    persist::{self, dsl::CidrMethods, DieselErrorFixCause},
    prefix_tree::PrefixTree,
};
use diesel::{PgConnection, QueryDsl, RunQueryDsl, SelectableHelper};
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

mod model;
mod component;

pub fn print_prefix(net: &Ipv6Net) -> Result {
    let mut buf = PrintedPrefixBuilder::default();
    let mut conn = persist::connect().map_err(pfxerr!(DbConnect))?;

    let tree = load_tree(&mut conn, net)?;
    writepfx!(
        &mut buf,
        "🌳 Tree data: {} 🍃{:?} 💰{:?} 💪{}%",
        tree.net,
        tree.merge_status,
        tree.priority_class,
        tree.confidence
    );

    let load_start = Instant::now();
    let measurements = load_relevant_measurements(&mut conn, net)?;
    writepfx!(
        &mut buf,
        "👀 {} /64 prefixes probed in this prefix (loaded from DB in {:?})",
        measurements.len(),
        load_start.elapsed(),
    );

    buf = buf.flush_section()?;

    let subnets = Subnets::new(*net, measurements).map_err(pfxerr!(SubnetSplit))?;
    for subnet in subnets.iter() {
        writepfx!(&mut buf,);
        writepfx!(&mut buf, "▶ Subnet: {}", subnet.subnet.network);

        if subnet.probe_count() == 0 {
            writepfx!(&mut buf, " No probes recorded.");
            continue;
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
        for (addr, item) in subnet.iter_lhrs() {
            let percent =
                (item.hit_count as i64 * 100i64).div_euclid(subnet.responsive_count() as i64);
            writepfx!(
                &mut buf,
                "  🚏 {} - {} hits ({}%)",
                addr,
                item.hit_count,
                percent
            );
        }
        writepfx!(&mut buf, " Weirdness:");
        for (typ, item) in subnet.iter_weirds() {
            writepfx!(&mut buf, "  🌪 {:?} - {} hits", typ, item.hit_count);
        }

        buf = buf.flush_section()?;
    }

    Ok(buf.into())
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

