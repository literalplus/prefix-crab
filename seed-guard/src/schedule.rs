use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::*;
use clap::Args;
use db_model::{
    persist::{dsl::CidrMethods, DieselErrorFixCause},
    prefix_tree::{AsNumber, MergeStatus},
    schema::{as_prefix, prefix_tree},
};
use diesel::{delete, upsert::excluded, Connection, ExpressionMethods, PgConnection, RunQueryDsl};
use itertools::Itertools;
use log::{error, info, warn};
use prefix_crab::loop_with_stop;
use tokio::time::{interval, Instant};
use tokio_util::sync::CancellationToken;

use crate::{
    as_changeset::{self, AsChangeset, AsSetEntry},
    as_filter_list,
};

#[derive(Args, Debug, Clone)]
#[group(id = "schedule")]
pub struct Params {
    /// How often to update AS data from the filesystem, in seconds (default = 6h).
    /// An immediate update can be triggered by restarting the application.
    #[arg(long, env = "RESEED_INTERVAL_SECS", default_value = "21600")]
    reseed_interval_secs: u64,

    #[arg(long, env = "AS_REPO_BASE_DIR", default_value = "./asn-ip/as")]
    as_repo_base_dir: PathBuf,

    /// Controls how the `as_filter_list` table is handled. The default is
    /// to treat it as an allow list, and only insert prefixes from these
    /// ASNs into the prefix tree (still maintining `as_prefix`). Setting
    /// this to `true` causes the system to perform a scan of the entire
    /// IPv6 internet.
    #[arg(long, env = "ASN_FILTER_IS_DENY_LIST", default_value = "false")]
    asn_filter_is_deny_list: bool,
}

pub async fn run(stop_rx: CancellationToken, params: Params) -> Result<()> {
    if !params
        .as_repo_base_dir
        .metadata()
        .map(|meta| meta.is_dir())
        .context("checking AS repo base dir")?
    {
        return Err(anyhow!(
            "AS repo base dir {:?} is not a directory",
            params.as_repo_base_dir
        ));
    }
    let as_dir = params.as_repo_base_dir.join("as");
    info!(
        "Automatic re-seed scheduled every {}s.",
        params.reseed_interval_secs
    );
    let mut trigger = interval(Duration::from_secs(params.reseed_interval_secs));
    loop_with_stop!(
        "analysis timer", stop_rx,
        trigger.tick() => tick((&params), (&as_dir)) as simple
    )
}

fn tick(params: &Params, as_dir: &Path) {
    if let Err(e) = do_tick(params, as_dir) {
        error!("Failed to perform scheduled re-seed due to {:?}", e);
    }
}

fn do_tick(params: &Params, as_dir: &Path) -> Result<()> {
    let mut conn = crate::persist::connect("guard - scheduler")?;
    let start = Instant::now();

    let filter = as_filter_list::fetch(&mut conn, params.asn_filter_is_deny_list)
        .context("loading AS filter list")?;
    let changes =
        as_changeset::determine(&mut conn, as_dir, &filter).context("determining AS set")?;

    try_save_changes(&mut conn, changes);

    info!("Re-seed completed in {}ms.", start.elapsed().as_millis());
    Ok(())
}

fn try_save_changes(conn: &mut PgConnection, changes: AsChangeset) {
    for change in changes.values() {
        let res = conn.transaction(|conn| save_change(conn, change));
        if let Err(e) = res {
            error!("Unable to save changes to AS {:?} due to: {:?}", change, e);
        }
    }
}

macro_rules! delete_all_below {
    ($conn:ident, $dsl: ident, $nets: ident) => {
        let mut statement = delete($dsl::table).into_boxed();

        for removed_net in $nets.iter() {
            statement = statement.or_filter($dsl::net.subnet_or_eq6(removed_net));
        }

        statement.execute($conn).fix_cause()?;
    };
}

fn save_change(conn: &mut PgConnection, change: &AsSetEntry) -> Result<()> {
    info!(" --- Saving changes to AS{}", change.asn);

    let removed = &change.removed;
    if !removed.is_empty() {
        info!(
            "Removing some prefixes of AS{} from prefix tree: {:?}",
            change.asn, removed
        );
        delete_all_below!(conn, prefix_tree, removed);
        delete_all_below!(conn, as_prefix, removed);
    }

    if !change.added.is_empty() {
        info!(
            "Adding new prefixes of AS{}: {:?}",
            change.asn, change.added
        );
        save_as_prefixes(conn, change).context("saving added AS prefixes")?;
        save_fresh_prefix_nodes(conn, change).context("saving fresh prefix nodes")?;
    }

    Ok(())
}

fn save_as_prefixes(conn: &mut PgConnection, change: &AsSetEntry) -> Result<()> {
    use db_model::schema::as_prefix::dsl::*;

    let tuples = change
        .added
        .iter()
        .map(|it| (net.eq6(it), asn.eq(change.asn)))
        .collect_vec();

    diesel::insert_into(as_prefix)
        .values(tuples)
        .on_conflict(net)
        .do_update()
        .set((deleted.eq(false), asn.eq(excluded(asn))))
        .execute(conn)
        .fix_cause()?;
    Ok(())
}

fn save_fresh_prefix_nodes(conn: &mut PgConnection, change: &AsSetEntry) -> Result<()> {
    use db_model::schema::prefix_tree::dsl::*;

    let tuples = change
        .added
        .iter()
        .map(|it| {
            (
                net.eq6(it),
                merge_status.eq(MergeStatus::UnsplitRoot),
                asn.eq(change.asn),
            )
        })
        .collect_vec();

    let inserted = diesel::insert_into(prefix_tree)
        .values(tuples)
        .on_conflict_do_nothing()
        .execute(conn)
        .fix_cause()?;

    if inserted != change.added.len() {
        warn!(
            "Some added prefixes for {:?} conflicted with existing prefix nodes, skipped {}.",
            change,
            change.added.len() - inserted
        );
    } else {
        info!("Created {} prefix nodes.", inserted);
    }

    Ok(())
}
