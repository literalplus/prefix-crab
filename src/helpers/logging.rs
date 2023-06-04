use anyhow::{Context, Result};
use clap::Args;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use flexi_logger::{colored_default_format, detailed_format, Logger, LoggerHandle, WriteMode};
use log::Level;

#[derive(Args)]
#[derive(Debug)]
#[group(id = "bootstrap")]
#[command(author, version, about)]
pub struct Params {
    #[clap(flatten)]
    verbose: Verbosity<InfoLevel>,

    /// Use a flexi_logger configuration file
    #[arg(long = "log-spec")]
    use_log_spec: bool,

    /// Path to log spec
    #[arg(long, value_name = "TOML FILE", default_value = "logspec.toml")]
    log_spec_file: std::path::PathBuf,
}


pub fn configure_from(params: &Params) -> Result<LoggerHandle> {
    // log_level() returns None iff verbosity < 0, i.e. being most quiet seems reasonable
    let cli_level = params.verbose.log_level()
        .unwrap_or(Level::Error);

    let log_builder = Logger::try_with_env_or_str(cli_level.to_string())
        .context("Failed to parse logger spec from env RUST_LOG or cli level")?
        .write_mode(WriteMode::Async)
        .format_for_stdout(colored_default_format)
        .format_for_files(detailed_format);

    return match (&params.use_log_spec, &params.log_spec_file) {
        (true, specfile_path) => log_builder
            .start_with_specfile(specfile_path)
            .with_context(|| format!("Failed to start logger with specfile {:?}", *specfile_path)),
        (false, _) => log_builder
            .start().context("Failed to start logger handle w/o specfile")
    };
}
