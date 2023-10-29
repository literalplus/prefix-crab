use clap::Args;

#[derive(Args, Clone, Debug)]
#[group(id = "rabbit")]
pub struct Params {
    /// URI for AMQP (RabbitMQ) server to connect to.
    /// Environment variable: AMQP_URI
    /// If a password is required, it is recommended to specify the URL over the environment or
    /// a config file, to avoid exposure in shell history and process list.
    #[arg(long, env = "AMQP_URI")]
    pub amqp_uri: String,

    /// Name of the exchange to publish probe requests to
    #[arg(long, default_value = "prefix-crab.probe-request")]
    pub request_exchange_name: String,

    /// Whether to pretty print JSON in RabbitMQ responses.
    #[arg(long, env = "PRETTY_PRINT")]
    pretty_print: bool,
}
