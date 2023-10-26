use std::{
    collections::HashSet,
    fs::{DirEntry, File, Metadata},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use db_model::persist::DieselErrorFixCause;
use diesel::{pg::PgRowByRowLoadingMode, prelude::*, PgConnection, QueryDsl};
use ipnet::{IpNet, Ipv6Net};
use log::{debug, warn, info};
use nohash_hasher::IntMap;
use prefix_crab::helpers::ip::ExpectV6;

use crate::as_filter_list::AsFilterList;

#[derive(Default, Debug)]
pub struct AsSetEntry {
    pub asn: u32,
    pub added: HashSet<Ipv6Net>,
    pub removed: Vec<Ipv6Net>,
}

impl AsSetEntry {
    fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty()
    }
}

pub type AsChangeset = IntMap<u32, AsSetEntry>;

pub fn determine(
    conn: &mut PgConnection,
    base_dir: &Path,
    filter: &AsFilterList,
) -> Result<AsChangeset> {
    if !base_dir.is_dir() {
        bail!("AS directory {:?} is not a directory", base_dir);
    }

    let mut indexed = read_ases_from_dir(base_dir, filter).context("determining present ASNs")?;
    extend_with_db_asns(conn, &mut indexed).context("loading removed ASNs")?;

    indexed.retain(|_, v| v.has_changes());

    Ok(indexed)
}

fn read_ases_from_dir(base_dir: &Path, filter: &AsFilterList) -> Result<IntMap<u32, AsSetEntry>> {
    let mut result = IntMap::default();
    let read_dir = base_dir.read_dir().context("reading base directory")?;
    let mut skipped = 0u64;
    for entry in read_dir {
        let entry = entry.context("iterating base directory")?;
        let meta = entry
            .metadata()
            .context("reading directory entry metadata")?;
        let file_name = entry.file_name().clone();
        if let Some(model) = to_model(entry, meta) {
            if filter.allows(model.asn) {
                result.insert(model.asn, model);
            } else {
                skipped += 1;
            }
        } else {
            debug!("Invalid AS dir {:?}", file_name);
        }
    }
    info!("Skipped {} entries due to AS filter list, kept {}", skipped, result.len());
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
            let mut entry = AsSetEntry {
                asn,
                ..Default::default()
            };
            entry.added.extend(&prefixes);
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
        .map_while(|it| it.ok())
        .filter(|it| !it.starts_with('#'))
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
            .select((asn, net, deleted))
            .load_iter::<(i64, IpNet, bool), PgRowByRowLoadingMode>(conn)
            .fix_cause()?
    };

    for res in iter {
        let (asn, net, deleted) = res.context("iterating previous ASNs from DB")?;
        let asn = asn as u32;
        let net = net.expect_v6();

        let entry = indexed.entry(asn).or_default();
        if entry.added.contains(&net) {
            if !deleted {
                // if it is in the "current prefixes", then it was not added, it is unchanged
                entry.added.remove(&net);
            }
        } else if !deleted {
            entry.removed.push(net);
        }
    }

    Ok(())
}
