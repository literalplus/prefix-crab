use std::{io::{BufWriter, Write}, time::Instant};

use anyhow::Result;
use clap::Args;
use db_model::{
    analyse::{subnet::Subnets, MeasurementTree},
    persist::{self, dsl::CidrMethods, DieselErrorFixCause},
    prefix_tree::PrefixTree,
};
use diesel::{PgConnection, QueryDsl, RunQueryDsl, SelectableHelper};
use ipnet::Ipv6Net;
use tuirealm::{AttrValue, Attribute, PollStrategy, Update};

use self::app::Model;

mod app;
mod components;

#[derive(Args, Clone)]
pub struct Params {
    #[clap(flatten)]
    persist: persist::Params,

    target_prefix: Ipv6Net,
}

pub fn handle(params: Params) -> Result<()> {
    persist::initialize(&params.persist)?;

    println!("Starting...");
    let mut model = Model::new(params.target_prefix)?;
    model.terminal.enter_alternate_screen()?;
    if let Err(e) = model.terminal.enable_raw_mode() {
        model.terminal.leave_alternate_screen()?;
        Err(e)?;
    }

    let res = do_run(&mut model);

    let _ = model.terminal.disable_raw_mode();
    let _ = model.terminal.leave_alternate_screen();

    res
}

fn do_run(model: &mut Model) -> Result<()> {
    while !model.quit {
        match model.app.tick(PollStrategy::Once) {
            Err(err) => {
                model
                    .app
                    .attr(
                        &Id::StatusBar,
                        Attribute::Text,
                        AttrValue::String(format!("Application error: {}", err)),
                    )
                    .unwrap();
            }
            Result::Ok(messages) if messages.len() > 0 => {
                model.redraw = true;
                for msg in messages.into_iter() {
                    let mut msg = Some(msg);
                    while msg.is_some() {
                        msg = model.update(msg);
                    }
                }
            }
            _ => {}
        }

        if model.redraw {
            model.view()?;
            model.redraw = false;
        }
    }
    Ok(())
}

fn print_prefix(net: &Ipv6Net) -> Result<String> {
    let mut buf = BufWriter::new(vec![]);
    let mut conn = persist::connect()?;

    let tree = load_tree(&mut conn, net)?;
    writeln!(
        &mut buf,
        "🌳 Tree data: {} 🍃{:?} 💰{:?} 💪{}%",
        tree.net, tree.merge_status, tree.priority_class, tree.confidence
    )?;

    let load_start = Instant::now();
    let measurements = load_relevant_measurements(&mut conn, net)?;
    writeln!(
        &mut buf,
        "👀 {} /64 prefixes probed in this prefix (loaded from DB in {:?})",
        measurements.len(),
        load_start.elapsed(),
    )?;

    let subnets = Subnets::new(*net, measurements)?;
    for subnet in subnets.iter() {
        writeln!(&mut buf)?;
        writeln!(&mut buf, "▶ Subnet: {}", subnet.subnet.network)?;

        if subnet.probe_count() == 0 {
            writeln!(&mut buf, " No probes recorded.")?;
            continue;
        }

        let responsive_percent =
            (subnet.responsive_count() as i64 * 100i64).div_euclid(subnet.probe_count() as i64);
        writeln!(
            &mut buf,
            " {} Probes, of these: (🔊{} 🔇{}) => {}% responsive",
            subnet.probe_count(),
            subnet.responsive_count(),
            subnet.unresponsive_count(),
            responsive_percent,
        )?;

        writeln!(&mut buf, " Last-Hop Routers:")?;
        for (addr, item) in subnet.iter_lhrs() {
            let percent =
                (item.hit_count as i64 * 100i64).div_euclid(subnet.responsive_count() as i64);
            writeln!(
                &mut buf,
                "  🚏 {} - {} hits ({}%)",
                addr, item.hit_count, percent
            )?;
        }
        writeln!(&mut buf, " Weirdness:")?;
        for (typ, item) in subnet.iter_weirds() {
            writeln!(&mut buf, "  🌪 {:?} - {} hits", typ, item.hit_count)?;
        }
    }

    Ok(String::from_utf8(buf.into_inner()?)?)
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

#[derive(Debug, PartialEq)]
pub enum Msg {
    AppClose,
    SetStatus(String),
    JustRedraw,
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum Id {
    Viewport,
    StatusBar,
}
