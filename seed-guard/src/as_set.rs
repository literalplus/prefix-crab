use std::{
    fs::{DirEntry, File, Metadata},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use db_model::persist::DieselErrorFixCause;
use diesel::{pg::PgRowByRowLoadingMode, prelude::*, PgConnection, QueryDsl};
use ipnet::{IpNet, Ipv6Net};
use log::warn;
use nohash_hasher::IntMap;
use prefix_crab::helpers::ip::ExpectV6;

#[derive(Default)]
pub struct AsSetEntry {
    pub asn: u32,
    pub present: Vec<Ipv6Net>,
    pub removed: Vec<Ipv6Net>,
}

pub fn determine(conn: &mut PgConnection, base_dir: &Path) -> Result<IntMap<u32, AsSetEntry>> {
    if !base_dir.is_dir() {
        bail!("AS repo base dir {:?} is not a directory", base_dir);
    }

    let mut indexed = read_ases_from_dir(base_dir).context("determining present ASNs")?;
    extend_with_db_asns(conn, &mut indexed).context("loading removed ASNs")?;

    Ok(indexed)
}

fn read_ases_from_dir(base_dir: &Path) -> Result<IntMap<u32, AsSetEntry>> {
    let mut result = IntMap::default();
    let read_dir = base_dir.read_dir().context("reading base directory")?;
    for entry in read_dir {
        let entry = entry.context("iterating base directory")?;
        let meta = entry
            .metadata()
            .context("reading directory entry metadata")?;
        if let Some(model) = to_model(entry, meta) {
            result.insert(model.asn, model);
        }
    }
    Ok(result)
}

fn to_model(entry: DirEntry, meta: Metadata) -> Option<AsSetEntry> {
    if !meta.is_dir() {
        return None;
    }
    let name_safe = entry.file_name().into_string().ok()?;
    let asn: u32 = name_safe.parse().ok()?;
    match read_prefixes(entry.path()) {
        Ok(prefixes) => {
            let mut entry = AsSetEntry::default();
            entry.present.extend_from_slice(&prefixes);
            Some(entry)
        }
        Err(e) => {
            warn!(
                "Failed to read prefixes file for {:?}: {:?}",
                entry.path(),
                e
            );
            None
        }
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

fn extend_with_db_asns(
    conn: &mut PgConnection,
    indexed: &mut IntMap<u32, AsSetEntry>,
) -> Result<()> {
    let iter = {
        use crate::schema::as_prefix::dsl::*;
        as_prefix
            .select((asn, net))
            .load_iter::<(i64, IpNet), PgRowByRowLoadingMode>(conn)
            .fix_cause()?
    };

    for res in iter {
        let (asn, net) = res.context("iterating previous ASNs from DB")?;
        let asn = asn as u32;
        let net = net.expect_v6();

        let entry = indexed.entry(asn).or_default();
        if !entry.present.contains(&net) {
            entry.removed.push(net);
        }
    }

    Ok(())
}
