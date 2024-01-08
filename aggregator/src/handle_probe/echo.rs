use anyhow::*;
use db_model::prefix_tree::{self, ContextOps};
use diesel::PgConnection;
use ipnet::Ipv6Net;
use log::{info, warn};
use queue_models::probe_response::EchoProbeResponse;

use crate::{
    analyse::{self, context, persist::UpdateAnalysis, split, EchoResult},
    schedule::FollowUpRequest,
};

use super::{archive, ProbeHandler};

impl ProbeHandler {
    pub(super) async fn handle_echo(&mut self, res: &EchoProbeResponse) -> Result<()> {
        archive::process(&mut self.conn, &res.target_net, res);

        let (interpretation, context) = interpret_and_save(&mut self.conn, res.target_net, res)?;

        if interpretation.needs_follow_up() {
            if let Some(id) = &context.analysis.pending_follow_up {
                let model = FollowUpRequest {
                    id: id.parse().context("Invalid TypeID stored in node")?,
                    prefix_tree: *context.node(),
                    follow_ups: interpretation.follow_ups,
                };
                info!("Requesting follow-up {} for {}.", model.id, res.target_net);
                self.follow_up_tx.send(model).await.context("sending follow-up")?;
            } else {
                warn!("Interpretation needs follow-up but it wasn't registered in the node");
            }
        } else {
            info!("No further follow-up necessary, scheduling split analysis.");
            split::process(&mut self.conn, context, &self.blocklist)?;
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
    let mut context = analyse::context::fetch(conn, tree_context)
        .context("fetch context for probe handling")?;

    let mut interpretation = analyse::echo::process(model);

    interpretation
        .update_analysis(conn, &mut context)
        .context("while saving analysis data")?;

    Ok((interpretation, context))
}
