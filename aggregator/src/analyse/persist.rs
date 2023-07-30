use anyhow::Result;

use diesel::dsl::*;
use diesel::prelude::*;

use diesel::PgConnection;

use crate::prefix_tree::ContextOps;

use crate::schema::split_analysis::dsl as analysis_dsl;


use super::context::Context;

pub fn begin(conn: &mut PgConnection, context: Context, split_prefix_len: u8) -> Result<Context> {
    insert_into(analysis_dsl::split_analysis)
        .values((
            analysis_dsl::tree_id.eq(&context.node().id),
        ))
        .execute(conn)?;
    super::context::fetch(conn, context.parent)
}

pub trait UpdateAnalysis {
    fn update_analysis(&self, conn: &mut PgConnection, context: &mut Context) -> Result<()>;
}
