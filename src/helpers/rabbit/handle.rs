use std::sync::Arc;

use amqprs::channel::{
    Channel, ExchangeDeclareArguments, QueueBindArguments, QueueDeclareArguments,
};
use amqprs::connection::{Connection, OpenConnectionArguments};
use anyhow::*;
use log::debug;
use chrono::Local;

pub struct RabbitHandle {
    connection: Arc<Connection>,
    channel: Channel,
}

impl RabbitHandle {
    pub async fn connect(amqp_uri: &str, conn_name: &str) -> Result<Self> {
        let mut connection_args = OpenConnectionArguments::try_from(amqp_uri)
            .with_context(|| format!("Invalid connection URI in {:?}", amqp_uri))?;
        connection_args.connection_name(&format!("{}@{}", conn_name, Local::now()));
        let connection = Connection::open(&connection_args)
            .await
            .with_context(|| format!("while opening RabbitMQ connection {:?}", amqp_uri))
            .with_context(|| "Maybe double-check credentials?")?;
        let channel = Self::create_channel(&connection).await?;
        Ok(RabbitHandle {
            connection: Arc::new(connection),
            channel,
        })
    }

    async fn create_channel(conn: &Connection) -> Result<Channel> {
        let channel = conn
            .open_channel(None)
            .await
            .with_context(|| "while opening RabbitMQ channel")?;
        debug!("Fresh RabbitMQ channel connected.");
        Ok(channel)
    }

    pub fn chan(&self) -> &Channel {
        &self.channel
    }

    /// Creates a new handle on the same connection but with a fresh channel.
    /// Channels should not be shared across tasks/threads as per upstream docs.
    pub async fn fork(&self) -> Result<RabbitHandle> {
        let channel = Self::create_channel(&self.connection).await?;
        let handle = RabbitHandle {
            connection: self.connection.clone(),
            channel,
        };
        Ok(handle)
    }
}

pub struct ConfigureRabbit<'han> {
    handle: &'han RabbitHandle,
}

impl<'han> ConfigureRabbit<'han> {
    pub fn new(handle: &'han RabbitHandle) -> Self {
        ConfigureRabbit { handle }
    }

    fn chan(&self) -> &Channel {
        self.handle.chan()
    }

    pub async fn declare_queue(&self, name: &str) -> Result<&ConfigureRabbit> {
        let args = QueueDeclareArguments::new(name).durable(true).finish();
        self.chan()
            .queue_declare(args)
            .await
            .with_context(|| format!("while declaring queue {}", name))?
            .expect("queue_declare returned None even though no_wait was false");
        Ok(self)
    }

    pub async fn declare_exchange(&self, name: &str, typ: &str) -> Result<&ConfigureRabbit> {
        let args = ExchangeDeclareArguments::new(name, typ)
            .durable(true)
            .finish();
        self.chan()
            .exchange_declare(args)
            .await
            .with_context(|| format!("while declaring exchange {}", name))?;
        Ok(self)
    }

    pub async fn bind_queue_to(
        &self,
        queue_name: &str,
        exchange_name: &str,
    ) -> Result<&ConfigureRabbit> {
        self.chan()
            .queue_bind(QueueBindArguments::new(queue_name, exchange_name, ""))
            .await
            .with_context(|| format!("while binding {}->{}", queue_name, exchange_name))?;
        Ok(self)
    }

    pub async fn bind_queue_routing(
        &self,
        queue_name: &str,
        exchange_name: &str,
        routing_key: &str,
    ) -> Result<&ConfigureRabbit> {
        self.chan()
            .queue_bind(QueueBindArguments::new(
                queue_name,
                exchange_name,
                routing_key,
            ))
            .await
            .with_context(|| format!("while binding {}->{}", queue_name, exchange_name))?;
        Ok(self)
    }
}
