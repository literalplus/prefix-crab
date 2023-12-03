use anyhow::{Context, Result};
use clap::Args;
use log::debug;

pub use self::caller::Caller;
pub use self::targets::TargetCollector;

mod caller;
mod targets;

#[derive(Args, Clone)]
#[group(id = "yarrp")]
pub struct Params {
    /// Source IPv6 address to use for zmap
    #[arg(long, env = "YARRP_SOURCE_ADDRESS")]
    source_address: String,

    /// Optional gateway MAC, needed if there is no default route via the specified interface
    #[arg(long, env = "GATEWAY_MAC")]
    gateway_mac: Option<String>,

    #[arg(long, env = "MIN_TTL", default_value = "2")]
    min_ttl: u8,

    #[arg(long, env = "MAX_TTL", default_value = "16")]
    max_ttl: u8,

    /// If yarrp receives a response with TTL > max_ttl, it expands the
    /// TTL range up to this value, which is called fill mode.
    /// This means that we can probe with a relatively low max_ttl of
    /// 16 and still trace hosts that need higher TTLs.
    #[arg(long, env = "FILL_MODE_MAX_TTL", default_value = "32")]
    fill_mode_max_ttl: u8,

    /// Don't probe low TTLs since they likely are just our "own" outgoing
    /// hops. 3 should be probably safe.
    #[arg(long, env = "NEIGHBORHOOD_MAX_TTL", default_value = "3")]
    neighborhood_max_ttl: u8,

    /// Name of the source interface to use for zmap
    #[arg(long, env = "INTERFACE")]
    interface: Option<String>,

    #[arg(long, env = "YARRP_RATE_PPS", default_value = "10")] // upstream default
    rate_pps: u16,

    #[arg(long, env = "YARRP_SHUTDOWN_WAIT_SECS", default_value = "23")]
    shutdown_wait_secs: u16,

    /// FQ path to yarrp binary
    #[arg(long, default_value = "/usr/local/bin/yarrp", env = "YARRP_BIN_PATH")]
    bin_path: String,

    /// FQ path to sudo binary
    #[arg(long, default_value = "/usr/bin/sudo")]
    sudo_path: String,
}

impl Params {
    pub fn to_caller_verifying_sudo(&self) -> Result<Caller> {
        let mut caller = self._make_caller()?;
        caller
            .verify_sudo_access()
            .with_context(|| "If not using NOPASSWD, you might need to re-run sudo manually.")?;
        Ok(caller)
    }

    fn _make_caller(&self) -> Result<Caller> {
        let mut caller = Caller::new(self.sudo_path.to_string(), self.bin_path.to_string());
        caller.setup(self);
        debug!("Using zmap caller: {:?}", caller);
        Ok(caller)
    }

    pub fn to_caller_assuming_sudo(&self) -> Result<Caller> {
        let mut caller = self._make_caller()?;
        caller.assume_sudo_access();
        Ok(caller)
    }
}
