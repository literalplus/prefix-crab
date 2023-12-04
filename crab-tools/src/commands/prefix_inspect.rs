use anyhow::Result;
use clap::Args;
use db_model::{
    analyse::{subnet::Subnets, MeasurementTree},
    persist::{self, dsl::CidrMethods, DieselErrorFixCause},
    prefix_tree::PrefixTree,
};
use diesel::{PgConnection, QueryDsl, RunQueryDsl, SelectableHelper};
use ipnet::Ipv6Net;

#[derive(Args, Clone)]
pub struct Params {
    #[clap(flatten)]
    persist: persist::Params,

    target_prefix: Ipv6Net,
}

pub fn handle(params: Params) -> Result<()> {
    persist::initialize(&params.persist)?;
    let mut conn = persist::connect()?;

    let tree = load_tree(&mut conn, &params.target_prefix)?;
    println!(
        " -> Tree data: {} / {:?} / {}%",
        tree.net, tree.merge_status, tree.confidence
    );

    let measurements = load_relevant_measurements(&mut conn, &params.target_prefix)?;
    println!(
        " -> {} /64 prefixes probed in this prefix",
        measurements.len()
    );

    let subnets = Subnets::new(params.target_prefix, measurements)?;
    for subnet in subnets.iter() {
        println!();
        println!(" # Subnet: {}", subnet.subnet.network);

        println!(
            "   {} Probes:",
            subnet.unresponsive_count() + subnet.responsive_count()
        );
        println!("    * {} responsive", subnet.responsive_count());
        println!("    * {} unresponsive", subnet.unresponsive_count());

        println!("   Last-Hop Routers:");
        for (addr, item) in subnet.iter_lhrs() {
            println!("    * {} - {} hits", addr, item.hit_count);
        }
        println!("   Weirdness:");
        for (typ, item) in subnet.iter_weirds() {
            println!("    * {:?} - {} hits", typ, item.hit_count);
        }
    }

    Ok(())
}

fn load_tree(conn: &mut PgConnection, target: &Ipv6Net) -> Result<PrefixTree> {
    use db_model::schema::prefix_tree::dsl::*;

    prefix_tree
        .filter(net.eq6(target))
        .select(PrefixTree::as_select())
        .first(conn)
        .fix_cause()
}

fn load_relevant_measurements(
    conn: &mut PgConnection,
    base_net: &Ipv6Net,
) -> Result<Vec<MeasurementTree>> {
    use db_model::schema::measurement_tree::dsl::*;

    measurement_tree
        .filter(target_net.subnet_or_eq6(base_net))
        .select(MeasurementTree::as_select())
        .load(conn)
        .fix_cause()
}
