use anyhow::*;
use log::warn;
use queue_models::probe_response::TraceResponse;
use tracing::{instrument, Span};

use crate::analyse::context::ContextFetchError;
use crate::analyse::persist::UpdateAnalysis;

use crate::analyse;
use db_model::prefix_tree::ContextOps;

use super::{archive, ProbeHandler};

impl ProbeHandler {
    #[instrument(skip_all, fields(net, id = %res.id))]
    pub(super) fn handle_trace(&mut self, res: &TraceResponse) -> Result<()> {
        let mut context = match analyse::context::fetch_by_follow_up(&mut self.conn, &res.id) {
            Err(ContextFetchError::NoMatchingAnalysis { id }) => {
                warn!("Received unexpected trace result: {}", id);
                return Ok(());
            }
            r => r?,
        };

        Span::current().record("net", format!("{}", context.node().net));

        archive::process(&mut self.conn, &context.node().net, res);

        let mut interpretation = analyse::trace::process(res);
        interpretation.update_analysis(&mut self.conn, &mut context)?;

        analyse::split::process(&mut self.conn, context, &self.blocklist).map_err(|e| anyhow!(e))
    }
}
