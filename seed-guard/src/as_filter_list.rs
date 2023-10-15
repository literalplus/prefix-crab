use anyhow::Result;
use db_model::persist::DieselErrorFixCause;
use diesel::{prelude::*, PgConnection, QueryDsl};
use nohash_hasher::IntSet;

pub struct AsFilterList {
    is_deny_list: bool,
    matched_entries: IntSet<u32>,
}

impl AsFilterList {
    pub fn allows(&self, asn: u32) -> bool {
        let contains = self.matched_entries.contains(&asn);

        //   contains    is_deny    allows
        //  ---------- ----------- --------
        //          0   0 (ALLOW)        0
        //          0   1 (DENY)         1
        //          1   0 (ALLOW)        1
        //          1   1 (DENY)         0
        // i.e. "exactly one", i.e. XOR

        contains ^ self.is_deny_list
    }
}

pub fn fetch(
    conn: &mut PgConnection,
    is_allow_list: bool,
) -> Result<AsFilterList> {
    use db_model::schema::as_filter_list::dsl::*;

    let all_asns: Vec<i64> = as_filter_list
        .select(asn)
        .distinct()
        .load(conn)
        .fix_cause()?;
    let matched_entries = all_asns
        .into_iter()
        .filter_map(|it| it.try_into().ok())
        .collect();
    Ok(AsFilterList {
        is_deny_list: is_allow_list,
        matched_entries,
    })
}
