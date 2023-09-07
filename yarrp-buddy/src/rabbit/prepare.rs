use anyhow::*;

use prefix_crab::helpers::rabbit::ConfigureRabbit;
pub use prefix_crab::helpers::rabbit::RabbitHandle;
use queue_models::{probe_request::TraceRequest, TypeRoutedMessage};

use super::Params;

pub async fn prepare(params: &Params) -> Result<RabbitHandle> {
    let handle = RabbitHandle::connect(params.amqp_uri.as_str(), "yarrp-buddy").await?;

    let queue_name = params.in_queue_name.as_str();
    let out_exchange_name = params.out_exchange_name.as_str();
    let in_exchange_name = params.in_exchange_name.as_str();
    let configure = ConfigureRabbit::new(&handle);

    configure
        .declare_queue(queue_name)
        .await?
        .declare_exchange(out_exchange_name, "direct")
        .await?
        .declare_exchange(in_exchange_name, "direct")
        .await?
        .bind_queue_routing(queue_name, in_exchange_name, TraceRequest::routing_key())
        .await?;

    Ok(handle)
}
