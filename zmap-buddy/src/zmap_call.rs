use anyhow::{Context, Result};
use clap::Args;
use log::debug;

pub use self::caller::Caller;
pub use self::targets::TargetCollector;

mod targets;
mod caller;

#[derive(Args)]
#[derive(Clone)]
#[group(id = "zmap")]
pub struct Params {
    /// Source IPv6 address to use for zmap
    #[arg(long, env = "SOURCE_ADDRESS")]
    source_address: String,

    /// Optional gateway MAC, needed if there is no default route via the specified interface
    #[arg(long, env = "GATEWAY_MAC")]
    gateway_mac: Option<String>,

    /// Name of the source interface to use for zmap
    #[arg(long, env = "INTERFACE")]
    interface: Option<String>,

    /// FQ path to zmap binary
    #[arg(long, default_value = "/usr/local/sbin/zmap")]
    bin_path: String,

    /// FQ path to sudo binary
    #[arg(long, default_value = "/usr/bin/sudo")]
    sudo_path: String,
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
        if let Some(interface_name) = &self.interface {
            caller.request_interface(interface_name.to_string());
        }
        if let Some(gateway_mac) = &self.gateway_mac {
            caller.request_gateway_mac(gateway_mac.to_string());
        }
        debug!("Using zmap caller: {:?}", caller);
        caller.push_source_address(self.source_address.to_string())?;
        Ok(caller)
    }

    pub fn to_caller_assuming_sudo(&self) -> Result<Caller> {
        let mut caller = self._make_caller()?;
        caller.assume_sudo_access();
        Ok(caller)
    }
}
