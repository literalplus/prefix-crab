use anyhow::{Context, Result};
use clap::Args;
use log::debug;

pub use self::caller::Caller;
pub use self::targets::TargetCollector;

mod targets;
mod caller;

// 64 is default for Linux and should be enough for most "reasonable" topologies
// https://networkengineering.stackexchange.com/a/2222
// https://www.rfc-editor.org/rfc/rfc1700 -> 64 is also the recommended default
pub const SENT_TTL: u8 = 64;

#[derive(Args)]
#[derive(Clone)]
#[group(id = "zmap")]
pub struct Params {
    /// Source IPv6 address to use for zmap
    #[arg(long, env = "ZMAP_SOURCE_ADDRESS")]
    source_address: String,

    /// Optional gateway MAC, needed if there is no default route via the specified interface
    #[arg(long, env = "GATEWAY_MAC")]
    gateway_mac: Option<String>,

    /// Name of the source interface to use for zmap
    #[arg(long, env = "INTERFACE")]
    interface: Option<String>,

    /// FQ path to zmap binary
    #[arg(long, default_value = "/usr/local/sbin/zmap", env = "ZMAP_BIN_PATH")]
    bin_path: String,

    /// FQ path to sudo binary
    #[arg(long, default_value = "/usr/bin/sudo")]
    sudo_path: String,

    #[arg(long, env = "ZMAP_RATE_PPS", default_value = "10")]
    rate_pps: u16,

    #[arg(long, env = "ZMAP_SHUTDOWN_WAIT_SECS", default_value = "23")]
    shutdown_wait_secs: u16,
}

impl Params {
    pub fn to_caller_verifying_sudo(&self) -> Result<Caller> {
        let mut caller = self._make_caller()?;
        caller.verify_sudo_access()
            .with_context(|| "If not using NOPASSWD, you might need to re-run sudo manually.")?;
        Ok(caller)
    }

    fn _make_caller(&self) -> Result<Caller> {
        let mut caller = Caller::new(
            self.sudo_path.to_string(), self.bin_path.to_string(),
        );
        debug!("Using zmap caller: {:?}", caller);
        caller.setup(self)?;
        Ok(caller)
    }

    pub fn to_caller_assuming_sudo(&self) -> Result<Caller> {
        let mut caller = self._make_caller()?;
        caller.assume_sudo_access();
        Ok(caller)
    }
}
