use anyhow::*;
use diesel::PgConnection;
use ipnet::Ipv6Net;
use log::{info, warn};
use queue_models::probe_response::EchoProbeResponse;

use crate::{
    analyse::{
        self,
        context::{self, ContextFetchError, ContextFetchResult},
        persist::UpdateAnalysis,
        split,
        EchoResult,
    },
    prefix_tree::{self, ContextOps},
    schedule::FollowUpRequest,
};

use super::{archive, ProbeHandler};

impl ProbeHandler {
    pub(super) fn handle_echo(&mut self, res: &EchoProbeResponse) -> Result<()> {
        archive::process(&mut self.conn, &res.target_net, res);

        let (interpretation, context) = interpret_and_save(&mut self.conn, res.target_net, res)?;

        if interpretation.needs_follow_up() {
            if let Some(id) = &context.analysis.pending_follow_up {
                let model = FollowUpRequest {
                    id: id.parse().context("Invalid TypeID stored in node")?,
                    prefix_tree: *context.node(),
                    follow_ups: interpretation.follow_ups,
                };
                info!("Requesting follow-up {}, split analysis delayed.", model.id);
                self.follow_up_tx.send(model).context("sending follow-up")?;
            } else {
                warn!("Interpretation needs follow-up but it wasn't registered in the node");
            }
        } else {
            info!("No further follow-up necessary, scheduling split analysis.");
            split::process(&mut self.conn, context)?;
        }

        Ok(())
    }
}

fn interpret_and_save(
    conn: &mut PgConnection,
    target_net: Ipv6Net,
    model: &EchoProbeResponse,
) -> Result<(EchoResult, context::Context)> {
    let tree_context =
        prefix_tree::context::fetch(conn, &target_net).context("fetching tree context")?;
    let mut context = fetch_or_begin_context(conn, tree_context)
        .context("fetch/begin context for probe handling")?;

    let mut interpretation = analyse::echo::process(model);

    interpretation
        .update_analysis(conn, &mut context)
        .context("while saving analysis data")?;

    Ok((interpretation, context))
}

fn fetch_or_begin_context(
    conn: &mut PgConnection,
    parent: prefix_tree::context::Context,
) -> ContextFetchResult {
    let result = analyse::context::fetch(conn, parent);
    if let Err(ContextFetchError::NoActiveAnalysis { parent }) = result {
        // TODO probably shouldn't tolerate this any more once we actually create these analyses
        return analyse::persist::begin(conn, parent);
    }
    result
}
