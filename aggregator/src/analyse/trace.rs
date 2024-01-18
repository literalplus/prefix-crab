use std::ops::{Deref, DerefMut};

use anyhow::Result;
use diesel::{query_builder::AsChangeset, ExpressionMethods, PgConnection, RunQueryDsl};
use queue_models::{
    probe_request::TraceRequestId,
    probe_response::{LastHop, TraceResponse, TraceResponseType, TraceResult as TR},
};

use crate::{analyse::WeirdType, persist::DieselErrorFixCause};

use super::{context, persist::UpdateAnalysis, Interpretation, LhrSource};

#[derive(Debug)]
pub struct TraceResult {
    id: TraceRequestId,
    parent: Interpretation,
}

impl TraceResult {
    fn new(id: TraceRequestId) -> Self {
        Self {
            id,
            parent: Interpretation::default(),
        }
    }
}

impl Deref for TraceResult {
    type Target = Interpretation;

    fn deref(&self) -> &Self::Target {
        &self.parent
    }
}

impl DerefMut for TraceResult {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.parent
    }
}

pub fn process(model: &TraceResponse) -> TraceResult {
    let mut result = TraceResult::new(model.id);

    for target in model.results.iter() {
        match target {
            TR::LastResponsiveHop(hop) => register_as_lhr(&mut result, hop),
            TR::NoResponse { target_addr } => result.count_unresponsive(&[*target_addr]),
        }
    }

    result
}

fn register_as_lhr(result: &mut TraceResult, hop: &LastHop) {
    use TraceResponseType as T;

    let source = match hop.response_type {
        T::DestinationUnreachable { kind } => match (&kind).try_into() {
            Ok(source) => source,
            Err(kind) => {
                let typ = match kind {
                    5 => WeirdType::DestUnreachFailedEgress,
                    6 => WeirdType::DestUnreachRejectRoute,
                    _ => WeirdType::DestUnreachOther,
                };
                result.register_weirds(&[hop.target_addr], typ);
                return;
            }
        },
        T::EchoReply => {
            let weird = if hop.target_ttl.is_some() {
                WeirdType::EchoReplyInTrace
            } else {
                WeirdType::DifferentEchoReplySource
            };
            result.register_weirds(&[hop.target_addr], weird);
            return;
        }
        T::TimeExceeded => LhrSource::Trace,
    };
    result.register_lhrs(&[hop.target_addr], hop.last_hop_addr, source)
}

impl UpdateAnalysis for TraceResult {
    fn update_analysis(
        &mut self,
        conn: &mut PgConnection,
        context: &mut context::Context,
    ) -> Result<()> {
        use crate::schema::split_analysis::dsl::*;

        self.parent.update_analysis(conn, context)?;
        diesel::update(split_analysis)
            .filter(id.eq(context.analysis.id))
            .filter(pending_follow_up.eq(self.id.to_string()))
            .set(ClearFollowUp::default())
            .execute(conn)
            .fix_cause()?;

        Ok(())
    }
}

#[derive(AsChangeset, Default)]
#[diesel(table_name=crate::schema::split_analysis)]
#[diesel(treat_none_as_null = true)]
struct ClearFollowUp {
    pending_follow_up: Option<String>,
}
