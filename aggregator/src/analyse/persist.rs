use anyhow::Result;

use diesel::dsl::*;
use diesel::prelude::*;

use diesel::PgConnection;

use crate::persist::dsl::CidrMethods;
use crate::prefix_tree;
use crate::prefix_tree::ContextOps;

use crate::schema::split_analysis::dsl as analysis_dsl;
use crate::persist::DieselErrorFixCause;


use super::context::Context;
use super::context::ContextFetchResult;

pub fn begin(conn: &mut PgConnection, parent: prefix_tree::Context) -> ContextFetchResult {
    insert_into(analysis_dsl::split_analysis)
        .values((
            analysis_dsl::tree_net.eq6(&parent.node().net),
        ))
        .execute(conn)
        .fix_cause()?;
    super::context::fetch(conn, parent)
}

pub trait UpdateAnalysis {
    fn update_analysis(&mut self, conn: &mut PgConnection, context: &mut Context) -> Result<()>;
}
