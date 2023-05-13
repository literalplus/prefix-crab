use amqprs::channel::{Channel, ExchangeDeclareArguments, QueueBindArguments, QueueDeclareArguments};
use amqprs::connection::{Connection, OpenConnectionArguments};
use anyhow::*;
use log::debug;

use super::Params;

pub struct RabbitHandle {
    _connection: Connection,
    channel: Channel,
}

pub async fn prepare(params: &Params) -> Result<RabbitHandle> {
    debug!("Setting up RabbitMQ receiver...");
    let connection_args = OpenConnectionArguments::try_from(
        params.amqp_uri.as_str(),
    ).with_context(|| format!("Invalid connection URI in {:?}", params))?;
    let queue_name = params.in_queue_name.as_str();
    let out_exchange_name = params.out_exchange_name.as_str();
    let in_exchange_name = params.in_exchange_name.as_str();

    let handle = RabbitHandle::connect(params, &connection_args).await?;
    handle
        .declare_queue(queue_name).await?
        .declare_fanout_exchange(out_exchange_name).await?
        .declare_fanout_exchange(in_exchange_name).await?
        .bind_queue_to(queue_name, in_exchange_name).await?;
    Ok(handle)
}

impl RabbitHandle {
    async fn connect(
        params: &Params, connection_args: &OpenConnectionArguments,
    ) -> Result<Self> {
        let connection = Connection::open(&connection_args)
            .await
            .with_context(|| format!("while opening RabbitMQ connection {:?}", params))
            .with_context(|| "Maybe double-check credentials?")?;
        let channel = connection.open_channel(None)
            .await
            .with_context(|| "while opening RabbitMQ channel")?;
        Ok(RabbitHandle { _connection: connection, channel })
    }

    async fn declare_queue(&self, name: &str) -> Result<&RabbitHandle> {
        let args = QueueDeclareArguments::new(name)
            .durable(true)
            .finish();
        self.channel.queue_declare(args)
            .await
            .with_context(|| format!("while declaring queue {}", name))?
            .expect("queue_declare returned None even though no_wait was false");
        Ok(self)
    }

    async fn declare_fanout_exchange(&self, name: &str) -> Result<&RabbitHandle> {
        let args = ExchangeDeclareArguments::new(name, "fanout")
            .durable(true)
            .finish();
        self.channel.exchange_declare(args).await
            .with_context(|| format!("while declaring exchange {}", name))?;
        Ok(self)
    }

    async fn bind_queue_to(&self, queue_name: &str, exchange_name: &str) -> Result<&RabbitHandle> {
        let routing_key = "echo";
        self.channel.queue_bind(QueueBindArguments::new(
            queue_name, exchange_name, routing_key,
        )).await.with_context(|| format!("while binding {}->{}", queue_name, exchange_name))?;
        Ok(self)
    }

    pub fn chan(&self) -> &Channel {
        &self.channel
    }
}
