use amqprs::channel::{Channel, ExchangeDeclareArguments, QueueBindArguments, QueueDeclareArguments};
use amqprs::connection::{Connection, OpenConnectionArguments};
use anyhow::*;
use log::debug;

pub struct RabbitHandle {
    _connection: Connection,
    channel: Channel,
}

impl RabbitHandle {
    pub async fn connect(
        amqp_uri: &str,
    ) -> Result<Self> {
        let connection_args = OpenConnectionArguments::try_from(amqp_uri)
            .with_context(|| format!("Invalid connection URI in {:?}", amqp_uri))?;
        let connection = Connection::open(&connection_args)
            .await
            .with_context(|| format!("while opening RabbitMQ connection {:?}", amqp_uri))
            .with_context(|| "Maybe double-check credentials?")?;
        let channel = connection.open_channel(None)
            .await
            .with_context(|| "while opening RabbitMQ channel")?;
        debug!("RabbitMQ channel connected.");
        Ok(RabbitHandle { _connection: connection, channel })
    }

    pub fn chan(&self) -> &Channel {
        &self.channel
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
        let args = QueueDeclareArguments::new(name)
            .durable(true)
            .finish();
        self.chan().queue_declare(args)
            .await
            .with_context(|| format!("while declaring queue {}", name))?
            .expect("queue_declare returned None even though no_wait was false");
        Ok(self)
    }

    pub async fn declare_exchange(&self, name: &str, typ: &str) -> Result<&ConfigureRabbit> {
        let args = ExchangeDeclareArguments::new(name, typ)
            .durable(true)
            .finish();
        self.chan().exchange_declare(args).await
            .with_context(|| format!("while declaring exchange {}", name))?;
        Ok(self)
    }

    pub async fn bind_queue_to(&self, queue_name: &str, exchange_name: &str) -> Result<&ConfigureRabbit> {
        let routing_key = "echo";
        self.chan().queue_bind(QueueBindArguments::new(
            queue_name, exchange_name, routing_key,
        )).await.with_context(|| format!("while binding {}->{}", queue_name, exchange_name))?;
        Ok(self)
    }
}
