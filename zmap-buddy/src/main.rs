use anyhow::{bail, Context};
use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use flexi_logger::{colored_default_format, detailed_format, Logger, LoggerHandle, WriteMode};
use human_panic::setup_panic;
use log::{info, Level};

mod zmap_call;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[clap(flatten)]
    verbose: Verbosity<InfoLevel>,

    /// Use a flexi_logger configuration file
    #[arg(long = "log-spec")]
    use_log_spec: bool,

    /// Path to log spec
    #[arg(long, value_name = "TOML FILE", default_value = "logspec.toml")]
    log_spec_file: std::path::PathBuf,

    #[arg(long = "🤪", hide = true)]
    use_zany: bool,

    #[command(subcommand)]
    command: cmd_logic::Commands,
}

fn main() -> anyhow::Result<()> {
    setup_panic!();

    // need explicit type annotation for IntelliJ
    let cli: Cli = Cli::parse();
    let logger_handle = configure_logging(&cli)
        .context("Unable to configure logging")?;

    if cli.use_zany {
        info!("Oh... I'm sorry... I'm just in a silly goofy mood 🤪");
        bail!("oop")
    }

    let command_result = cmd_logic::handle(cli.command);

    // Important with non-direct write mode
    // Handle needs to be kept alive until end of program
    logger_handle.flush();

    command_result
}

fn configure_logging(cli: &Cli) -> anyhow::Result<LoggerHandle> {
    // log_level() returns None iff verbosity < 0, i.e. being most quiet seems reasonable
    let cli_level = cli.verbose.log_level()
        .unwrap_or(Level::Error);

    let log_builder = Logger::try_with_env_or_str(cli_level.to_string())
        .context("Failed to parse logger spec from env RUST_LOG or cli level")?
        // TODO: switch to async in release builds https://stackoverflow.com/a/39205417 ?
        .write_mode(WriteMode::BufferAndFlush)
        .format_for_stdout(colored_default_format)
        .format_for_files(detailed_format);

    return match (&cli.use_log_spec, &cli.log_spec_file) {
        (true, specfile_path) => log_builder
            .start_with_specfile(specfile_path)
            .with_context(|| format!("Failed to start logger with specfile {:?}", *specfile_path)),
        (false, _) => log_builder
            .start().context("Failed to start logger handle w/o specfile")
    };
}

mod cmd_logic {
    use anyhow::Context;
    use clap::{Args, Subcommand};
    use log::debug;

    pub fn handle(cmd: Commands) -> anyhow::Result<()> {
        let command_result = match cmd {
            Commands::SingleCall(data) => handle_single(data),
        };
        debug!("Finished command execution. Result: {:?}", command_result);
        command_result
    }

    fn handle_single(cmd: SingleCallData) -> anyhow::Result<()> {
        let mut caller = super::zmap_call::Caller::new(cmd.sudo_path, cmd.bin_path);
        debug!("Using zmap caller: {:?}", caller);
        caller.verify_sudo_access()
            .with_context(|| "If not using NOPASSWD, you might need to re-run sudo manually.")?;

        let targets = if cmd.target_addresses.is_empty() {
            [
                "fdf9:d3a4:2fff:96ec::a", "fd00:aff1:3::a", "fd00:aff1:3::3a",
                "fd00:aff1:3::c", "fd00:aff1:678::b", "2a02:8388:8280:ec80:3a43:7dff:febe:998",
                "2a02:8388:8280:ec80:3a43:7dff:febe:999"
            ].iter().map(|static_str| static_str.to_string()).collect()
        } else {
            cmd.target_addresses
        };

        caller.push_targets_vec(targets)
            .with_context(|| "Failed to write target addresses")?;
        caller.push_source_address(cmd.source_address)?;
        caller.consume_run()
    }

    #[derive(Subcommand)]
    pub enum Commands {
        /// Perform a single call to zmap.
        SingleCall(SingleCallData),
    }

    #[derive(Args)]
    pub struct SingleCallData {
        #[arg(long)]
        source_address: String,

        /// FQ path to zmap binary
        #[arg(long, default_value = "/usr/local/sbin/zmap")]
        bin_path: String,

        /// FQ path to sudo binary
        #[arg(long, default_value = "/usr/bin/sudo")]
        sudo_path: String,

        target_addresses: Vec<String>,
    }
}
