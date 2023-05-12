use anyhow::Result;
use clap::Subcommand;
use log::debug;

pub mod one_shot;
pub mod prefix_scan;
pub mod rabbitmq_listen;

pub fn handle(cmd: Commands) -> Result<()> {
    let command_result = match cmd {
        Commands::OneShot(data) => one_shot::handle(data),
        Commands::PrefixScan(data) => prefix_scan::handle(data),
        Commands::RabbitMqListen(data) => rabbitmq_listen::handle(data),
    };
    debug!("Finished command execution. Result: {:?}", command_result);
    command_result
}

#[derive(Subcommand)]
pub enum Commands {
    /// Perform a single call to zmap, probing given target addresses.
    OneShot(one_shot::Params),

    /// Scan one level of prefix for responsive sub-prefixes.
    PrefixScan(prefix_scan::Params),

    /// Listen to scanning commands from RabbitMQ, probe batched in the background, and write the
    /// results back to RabbitMQ.
    RabbitMqListen(rabbitmq_listen::Params),
}
