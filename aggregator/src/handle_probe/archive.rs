use std::fmt::Debug;

use anyhow::*;
use diesel::insert_into;
use diesel::prelude::*;
use ipnet::Ipv6Net;
use log::{trace, warn};

use serde::Serialize;

use crate::persist::dsl::CidrMethods;
use crate::schema::response_archive::dsl::*;

pub fn process<T>(conn: &mut PgConnection, target_net: &Ipv6Net, model: &T)
where
    T: Serialize + Debug,
{
    // Note: This could technically be separated into a different component, then that should
    // be independent of any processing errors (giving us a decent chance at reprocessing if
    // combined with some sort of success flag/DLQ)
    if let Err(e) = archive_response(conn, target_net, model) {
        warn!("Unable to archive response: {:?} - due to {}", &model, e);
    } else {
        trace!("Response successfully archived.");
    }
}

fn archive_response<T>(
    conn: &mut PgConnection,
    target_net: &Ipv6Net,
    model: &T,
) -> Result<(), Error>
where
    T: Serialize,
{
    let model_jsonb =
        serde_json::to_value(model).with_context(|| "failed to serialize to json for archiving")?;
    insert_into(response_archive)
        .values((path.eq6(target_net), data.eq(model_jsonb)))
        .execute(conn)
        .with_context(|| "while trying to insert into response archive")?;
    Ok(())
}
