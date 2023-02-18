use std::{fs, io};
use std::borrow::Cow;
use std::io::{BufRead, Read};
use std::net::Ipv6Addr;
use std::process::{Command, Stdio};

use anyhow::{bail, Context};
use log::{debug, log_enabled};
use log::Level::Debug;

/// Base config for calling zmap
#[derive(Debug)]
pub struct Caller {
    cmd: Command,
    bin_path: String,
}

impl Caller {
    pub fn new(sudo_path: String, bin_path: String) -> Self {
        let mut cmd = Command::new(sudo_path);
        cmd.arg("--non-interactive").arg("--").arg(bin_path.to_string());
        return Caller { cmd, bin_path };
    }

    pub fn verify_sudo_access(&self) -> anyhow::Result<()> {
        let mut check_cmd = Command::new(self.cmd.get_program());
        check_cmd.arg("--non-interactive").arg("--list").arg("--").arg(self.bin_path.to_string());
        let mut child = check_cmd.spawn()
            .with_context(|| "Failed to spawn sudo check process")?;
        let exit_status = child.wait()
            .with_context(|| "Failed to wait for sudo check process to exit")?;
        debug!("Sudo checker exited with {}", exit_status);
        if !exit_status.success() {
            bail!("No sudo access detected, {}", exit_status)
        } else {
            Ok(())
        }
    }

    /// Pushes an IPv6 source address to the generated command. Should only be called once.
    pub fn push_source_address(&mut self, source_address_string: String) -> anyhow::Result<()> {
        let parsed_source_address: Ipv6Addr = source_address_string.parse()
            .with_context(|| format!("Failed to parse source IPv6: {}", source_address_string))?;
        self.cmd.arg(format!("--ipv6-source-ip={}", parsed_source_address));
        Ok(())
    }

    /// Runs the configured command, consuming this instance.
    pub fn consume_run(mut self) -> anyhow::Result<()> {
        self.set_base();
        self.set_logging()?;
        self.set_targets()?;
        self.do_call()
    }

    fn do_call(&mut self) -> anyhow::Result<()> {
        if log_enabled!(Debug) {
            let args: Vec<Cow<'_, str>> = self.cmd.get_args()
                .map(|os_str| os_str.to_string_lossy())
                .collect();
            debug!("Calling zmap with arguments: {}", args.join(" "));
        }

        let mut child = self.cmd.spawn()
            .with_context(|| "Failed to spawn zmap process")?;

        self.prefix_out_fd(&mut child.stdout);
        self.prefix_out_fd(&mut child.stderr);

        let exit_status = child.wait()
            .with_context(|| "Failed to wait for child to exit")?;

        if exit_status.success() {
            debug!("zmap call exited successfully");
            Ok(())
        } else {
            bail!("zmap call exited with non-successful status {:?}", exit_status)
        }
    }

    fn prefix_out_fd<R: Read + Send + 'static>(&self, fd: &mut Option<R>) {
        let taken = fd.take().expect("Failed to open output stream of child");
        std::thread::spawn(move || {
            let reader = io::BufReader::new(taken);
            for line in reader.lines() {
                if let Ok(ln) = line {
                    println!(" -[zmap]- {}", ln)
                }
            }
        });
    }

    fn set_base(&mut self) {
        self.cmd
            .arg("--bandwidth=10K")
            .arg("--max-targets=10")
            .arg("--output-file=out/results.csv")
            //.arg("--dryrun")
            .arg("--verbosity=5")
            .arg("--cooldown-time=4") // wait for responses for n secs after sending
            //.arg("--seed=n") // TODO this permutes addresses?
            //.arg("--gateway-mac=addr")
            //.arg("--source-mac=addr")
            //.arg("--interface=name")
            .arg("--probe-module=icmp6_echoscan")
            .arg("--probe-ttl=255")

            .arg("--output-fields=type,code,original_ttl,orig-dest-ip,classification")
            .arg("--output-module=csv")

            .arg("--disable-syslog")

            //.arg("--cores=idx,idx,idx") // cores to pin to
            .arg("--sender-threads=1")
            .arg("--ignore-blacklist-errors");


        self.cmd.stdin(Stdio::piped()); // Allow password entry for sudo (local debug)
        self.cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        self.cmd.env_clear();
    }

    fn set_logging(&mut self) -> anyhow::Result<()> {
        let log_dir = "out/zmap-logs";
        fs::create_dir_all(log_dir)
            .with_context(|| format!("Unable to create zmap log directory {:?}", log_dir))?;

        //.arg(format!("--log-directory={}", zmap_log_dir))
        self.cmd.arg(format!("--log-file={}/latest.log", log_dir));

        Ok(())
    }

    fn set_targets(&mut self) -> anyhow::Result<()> {
        let target_addrs = "fdf9:d3a4:2fff:96ec::a\nfd00:aff1:3::a\nfd00:aff1:3::3a\nfd00:aff1:3::c\n";
        let addr_lst_path = "out/zmap-addr-list.txt";
        fs::write(addr_lst_path, target_addrs)
            .with_context(|| format!("Unable to write address list to {:?}", addr_lst_path))?;

        self.cmd.arg(format!("--ipv6-target-file={}", addr_lst_path)); // or - for stdin

        Ok(())
    }
}
