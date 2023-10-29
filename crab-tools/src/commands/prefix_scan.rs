use amqprs::channel::BasicPublishArguments;
use amqprs::BasicProperties;
use anyhow::*;
use clap::Args;
use futures::executor;
use ipnet::Ipv6Net;
use log::info;

use prefix_crab::helpers::rabbit::RabbitHandle;
use queue_models::RoutedMessage;
use queue_models::probe_request::EchoProbeRequest;

use crate::rabbit;

#[derive(Args, Clone)]
pub struct Params {
    #[clap(flatten)]
    rabbit: rabbit::Params,

    target_prefix: Ipv6Net,
}

pub fn handle(params: Params) -> Result<()> {
    let sender = RabbitSender {
        exchange_name: params.rabbit.request_exchange_name.to_string(),
    };
    let rabbit_handle = tokio::spawn(sender.run(params.clone()));

    executor::block_on(rabbit_handle)??;

    info!("Requested to scan prefix {}.", params.target_prefix);
    Ok(())
}

struct RabbitSender {
    exchange_name: String,
}

impl RabbitSender {
    async fn run(self, params: Params) -> Result<()> {
        let handle = RabbitHandle::connect(params.rabbit.amqp_uri.as_str(), "crab-tools").await?;
        let msg = EchoProbeRequest {
            target_net: params.target_prefix,
        };

        self.publish(msg, handle).await
    }

    async fn publish(&self, msg: EchoProbeRequest, handle: RabbitHandle) -> Result<()> {
        let args = BasicPublishArguments::new(&self.exchange_name, msg.routing_key());
        let bin = serde_json::to_vec_pretty(&msg)
            .with_context(|| format!("during serialisation of {:?}", msg))?;
        handle
            .chan()
            .basic_publish(BasicProperties::default(), bin, args)
            .await
            .with_context(|| "during publish")
    }
}
