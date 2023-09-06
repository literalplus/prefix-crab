use std::ops::DerefMut;

use anyhow::*;

use diesel::prelude::*;
use diesel::PgConnection;
use queue_models::probe_request::TraceRequestId;

use crate::analyse::context::Context;
use crate::analyse::persist::UpdateAnalysis;
use crate::analyse::EchoResult;
use crate::persist::DieselErrorFixCause;

use crate::schema::split_analysis::pending_follow_up;

impl UpdateAnalysis for EchoResult {
    fn update_analysis(&mut self, conn: &mut PgConnection, context: &mut Context) -> Result<()> {
        self.deref_mut().update_analysis(conn, context)?;

        if self.needs_follow_up() {
            let id = TraceRequestId::new();
            context.analysis.pending_follow_up = Some(id.to_string());
            diesel::update(&context.analysis)
                .set(pending_follow_up.eq(id.to_string()))
                .execute(conn)
                .fix_cause()?;
        }

        Ok(())
    }
}
