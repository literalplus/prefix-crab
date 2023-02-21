use anyhow::{bail, Context, Result};
use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use flexi_logger::{colored_default_format, detailed_format, Logger, LoggerHandle, WriteMode};
use human_panic::setup_panic;
use log::{info, Level};

mod cmd_logic;
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

fn main() -> Result<()> {
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

fn configure_logging(cli: &Cli) -> Result<LoggerHandle> {
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
