use anyhow::*;
use diesel::insert_into;
use diesel::prelude::*;
use log::{trace, warn};

use queue_models::echo_response::EchoProbeResponse;

use crate::model::PrefixPath;
use crate::schema::response_archive::dsl::*;

pub fn process(
    conn: &mut PgConnection, target_net: &PrefixPath, model: &EchoProbeResponse,
) {
    // Note: This could technically be separated into a different component, then that should
    // be independent of any processing errors (giving us a decent chance at reprocessing if
    // combined with some sort of success flag/DLQ)
    if let Err(e) = archive_response(conn, target_net, model) {
        warn!("Unable to archive response: {:?} - due to {}", &model, e);
    } else {
        trace!("Response successfully archived.");
    }
}

fn archive_response(
    conn: &mut PgConnection, target_net: &PrefixPath, model: &EchoProbeResponse,
) -> Result<(), Error> {
    let model_jsonb = serde_json::to_value(model)
        .with_context(|| "failed to serialize to json for archiving")?;
    insert_into(response_archive)
        .values((
            path.eq(target_net),
            data.eq(model_jsonb),
        ))
        .execute(conn)
        .with_context(|| "while trying to insert into response archive")?;
    Ok(())
}
