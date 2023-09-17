use std::{
    collections::{HashMap, HashSet},
    fs::{DirEntry, File, Metadata},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use anyhow::{Result, Context, bail};
use db_model::persist::DieselErrorFixCause;
use diesel::{pg::PgRowByRowLoadingMode, prelude::*, PgConnection, QueryDsl};
use ipnet::{IpNet, Ipv6Net};
use log::warn;
use prefix_crab::helpers::ip::ExpectV6;

pub enum AsSetEntry {
    Present { asn: u32, prefixes: Vec<Ipv6Net> },
    Removed { asn: u32, prefixes: Vec<Ipv6Net> },
}

impl AsSetEntry {
    fn asn(&self) -> &u32 {
        match self {
            AsSetEntry::Present { asn, prefixes: _ } => asn,
            AsSetEntry::Removed { asn, prefixes: _ } => asn,
        }
    }

    fn prefixes(&mut self) -> &mut Vec<Ipv6Net> {
        match self {
            AsSetEntry::Present { asn: _, prefixes } => prefixes,
            AsSetEntry::Removed { asn: _, prefixes } => prefixes,
        }
    }
}

pub fn determine(conn: &mut PgConnection, base_dir: &Path) -> Result<Vec<AsSetEntry>> {
    if !base_dir.is_dir() {
        bail!("AS repo base dir {:?} is not a directory", base_dir);
    }

    let present = read_ases_from_dir(base_dir).context("determining present ASNs")?;
    let absent = find_absent_asns_from_db(conn, &present).context("loading removed ASNs")?;
    let chained = present.into_iter().chain(absent.into_iter()).collect();

    Ok(chained)
}

fn read_ases_from_dir(base_dir: &Path) -> Result<Vec<AsSetEntry>> {
    let mut result = vec![];
    let read_dir = base_dir.read_dir().context("reading base directory")?;
    for entry in read_dir {
        let entry = entry.context("iterating base directory")?;
        let meta = entry
            .metadata()
            .context("reading directory entry metadata")?;
        if let Some(model) = to_model(entry, meta) {
            result.push(model)
        }
    }
    Ok(result)
}

fn to_model(entry: DirEntry, meta: Metadata) -> Option<AsSetEntry> {
    if meta.is_dir() {
        let name_safe = entry.file_name().into_string().ok()?;
        let asn: u32 = name_safe.parse().ok()?;
        match read_prefixes(entry.path()) {
            Ok(prefixes) => Some(AsSetEntry::Present { asn, prefixes }),
            Err(e) => {
                warn!("Failed to read prefixes file for {:?}: {:?}", entry.path(), e);
                None
            }
        }
    } else {
        None
    }
}

fn read_prefixes(base_path: PathBuf) -> Result<Vec<Ipv6Net>> {
    let path = base_path.join("ipv6-aggregated.txt");
    let file = File::open(path.clone()).context("opening file")?;
    let result = BufReader::new(file)
        .lines()
        .into_iter()
        .filter_map(|it| it.ok())
        .filter(|it| !it.starts_with("#"))
        .filter_map(|it| it.parse::<Ipv6Net>().ok())
        .collect();
    Ok(result)
}

fn find_absent_asns_from_db(
    conn: &mut PgConnection,
    present: &[AsSetEntry],
) -> Result<Vec<AsSetEntry>> {
    let present_asns: HashSet<u32> = present.iter().map(|it| *it.asn()).collect();
    let mut result: HashMap<u32, AsSetEntry> = HashMap::new();

    // FIXME we could just use this info already for the next step!
    let iter = {
        use crate::schema::as_prefix::dsl::*;
        as_prefix
            .select((asn, net))
            .load_iter::<(i64, IpNet), PgRowByRowLoadingMode>(conn)
            .fix_cause()?
    };

    for res in iter {
        let (asn, net) = res.context("iterating previus ASNs from DB")?;
        let asn = asn as u32;
        let net = net.expect_v6();
        if !present_asns.contains(&asn) {
            let entry = result.entry(asn).or_insert(AsSetEntry::Removed {
                asn,
                prefixes: vec![],
            });
            entry.prefixes().push(net);
        }
    }

    Ok(result.into_values().collect())
}
