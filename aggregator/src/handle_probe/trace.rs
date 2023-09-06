use anyhow::*;
use log::warn;
use queue_models::probe_response::TraceResponse;

use crate::analyse::context::ContextFetchError;
use crate::analyse::persist::UpdateAnalysis;

use crate::analyse;
use crate::prefix_tree::ContextOps;

use super::{archive, ProbeHandler};

impl ProbeHandler {
    pub(super) fn handle_trace(&mut self, res: &TraceResponse) -> Result<()> {
        let mut context = match analyse::context::fetch_by_follow_up(&mut self.conn, &res.id) {
            Err(ContextFetchError::NoMatchingAnalysis { id }) => {
                warn!("Received unexpected trace result: {}", id);
                return Ok(());
            }
            r => r?,
        };

        archive::process(&mut self.conn, &context.node().net, res);

        let mut interpretation = analyse::trace::process(res);
        interpretation.update_analysis(&mut self.conn, &mut context)?;

        analyse::split::process(&mut self.conn, context).map_err(|e| anyhow!(e))
    }
}
