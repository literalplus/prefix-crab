use std::borrow::Cow;
use std::io::{self, BufRead, Read};
use std::net::Ipv6Addr;
use std::process::{ChildStdout, Command, Stdio};

use crate::schedule::ProbeResponse;
use anyhow::{bail, Context, Result};
use log::Level::Debug;
use log::{debug, error, log_enabled, trace, warn};
use regex::Regex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use super::targets::TargetCollector;

/// Base config for calling zmap
#[derive(Debug)]
pub struct Caller {
    cmd: Command,
    bin_path: String,
    sudo_verified: bool,
    response_tx: Option<UnboundedSender<ProbeResponse>>,
}

impl Caller {
    pub fn new(sudo_path: String, bin_path: String) -> Self {
        let mut cmd = Command::new(sudo_path);
        cmd.arg("--non-interactive").arg("--").arg(&bin_path);
        Caller {
            cmd,
            bin_path,
            sudo_verified: false,
            response_tx: None,
        }
    }

    pub fn verify_sudo_access(&mut self) -> Result<()> {
        if self.sudo_verified {
            trace!("sudo access already checked and present.");
            return Ok(());
        }
        let mut check_cmd = Command::new(self.cmd.get_program());
        check_cmd
            .arg("--non-interactive")
            .arg("--list")
            .arg("--")
            .arg(&self.bin_path);
        check_cmd.stdout(Stdio::null()).stderr(Stdio::null());
        let mut child = check_cmd
            .spawn()
            .with_context(|| "Failed to spawn sudo check process")?;
        let exit_status = child
            .wait()
            .with_context(|| "Failed to wait for sudo check process to exit")?;
        debug!("Sudo checker exited with {}", exit_status);
        if !exit_status.success() {
            bail!("No sudo access detected, {}", exit_status)
        } else {
            self.sudo_verified = true;
            Ok(())
        }
    }

    pub fn assume_sudo_access(&mut self) {
        self.sudo_verified = true;
    }

    /// Pushes an IPv6 source address to the generated command. Should only be called once.
    pub fn push_source_address(&mut self, source_address_string: String) -> Result<()> {
        let parsed_source_address: Ipv6Addr = source_address_string
            .parse()
            .with_context(|| format!("Failed to parse source IPv6: {}", source_address_string))?;
        self.cmd
            .arg(format!("--ipv6-source-ip={}", parsed_source_address));
        Ok(())
    }

    /// Request responses to be captured instead of just printed.
    /// Responses will be provided to the returned [Receiver].
    /// The sender will be dropped once zmap closes stdout (i.e. exits).
    pub fn request_responses(&mut self) -> UnboundedReceiver<ProbeResponse> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.response_tx = Some(tx);
        rx
    }

    pub fn request_interface(&mut self, interface_name: String) {
        self.cmd.arg(format!("--interface={}", interface_name));
    }

    pub fn request_gateway_mac(&mut self, gateway_mac: String) {
        self.cmd.arg(format!("--gateway-mac={}", gateway_mac));
    }

    /// Runs the configured command, consuming this instance.
    pub fn consume_run(mut self, targets: TargetCollector) -> Result<()> {
        self.set_base();
        self.push_targets(targets)?;
        self.do_call()
    }

    fn push_targets(&mut self, mut collector: TargetCollector) -> Result<()> {
        // Collector moved intentionally; Writing to it while the program is running
        // has undefined effect, so we prohibit that.
        collector.flush()?;
        let path_as_str = collector
            .path
            .to_str()
            .with_context(|| "Non-UTF-8 path provided for targets file")?;
        self.cmd.arg(format!("--ipv6-target-file={}", path_as_str));
        Ok(())
    }

    fn do_call(&mut self) -> Result<()> {
        if log_enabled!(Debug) {
            let args: Vec<Cow<'_, str>> = self
                .cmd
                .get_args()
                .map(|os_str| os_str.to_string_lossy())
                .collect();
            debug!("Calling zmap with arguments: {}", args.join(" "));
        }

        let mut child = self
            .cmd
            .spawn()
            .with_context(|| "Failed to spawn zmap process")?;

        self.watch_logger_fd(&mut child.stderr);
        match self.response_tx.take() {
            Some(tx) => self.watch_stdout(&mut child.stdout, tx),
            None => self.watch_logger_fd(&mut child.stdout),
        }

        let exit_status = child
            .wait()
            .with_context(|| "Failed to wait for child to exit")?;

        if exit_status.success() {
            debug!("zmap call exited successfully");
            Ok(())
        } else {
            bail!(
                "zmap call exited with non-successful status {:?}",
                exit_status
            )
        }
    }

    fn watch_logger_fd<R: Read + Send + 'static>(&self, fd: &mut Option<R>) {
        let taken = fd.take().expect("Failed to open output stream of child");
        std::thread::spawn(move || {
            let reader = io::BufReader::new(taken);
            let logger_line_re = Regex::new(r"^[a-zA-Z]{3} \d{1,2} [\d:.]+ \[(?P<level>[A-Z]+)]")
                .expect("Unable to compile logger line regex");
            let mut locs = logger_line_re.capture_locations();
            for line in reader.lines().flatten() {
                let line_slice = line.as_str();
                if logger_line_re.captures_read(&mut locs, line_slice).is_some() {
                    let (start, end) = locs.get(1).expect("First capture");
                    match &line[start..end] {
                        "DEBUG" => trace!("zmap: {}", line),
                        "FATAL" => error!("zmap: {}", line),
                        _ => debug!("zmap: {}", line),
                    }
                } else {
                    trace!("zmap: {}", line);
                }
            }
        });
    }

    fn watch_stdout(&mut self, fd: &mut Option<ChildStdout>, tx: UnboundedSender<ProbeResponse>) {
        let taken_fd = fd.take().expect("Failed to open output stream of child");
        std::thread::spawn(move || {
            let mut reader = csv::Reader::from_reader(taken_fd);
            for record_res in reader.deserialize::<ProbeResponse>() {
                match record_res {
                    Ok(record) => {
                        trace!("[[zmap result]] {:?}", record);
                        if let Err(e) = tx.send(record) {
                            warn!(
                                "Unable to send response over channel; \
                            maybe the receiver disconnected? {}",
                                e
                            );
                            break;
                        }
                    }
                    Err(e) => warn!("Failed to parse CSV record from zmap: {}", e),
                }
            }
            trace!("Done reading from zmap stdout");
            drop(tx);
        });
    }

    fn set_base(&mut self) {
        self.cmd
            .arg("--bandwidth=10K") // TODO use "rate" in pps instead for consistency with yarrp?
            .arg("--verbosity=5")
            .arg("--cooldown-time=4") // wait for responses for n secs after sending
            // TODO: Permute addresses manually, as --seed is not supported for v6
            .arg("--probe-module=icmp6_echoscan")
            .arg("--probe-ttl=255")
            .arg("--output-fields=type,code,original_ttl,orig-dest-ip,saddr,classification")
            .arg("--output-module=csv")
            .arg("--disable-syslog")
            //.arg("--cores=idx,idx,idx") // cores to pin to
            .arg("--sender-threads=1")
            .arg("--ignore-blacklist-errors");
        // TODO: check that blocklist is actually used with --ipv6-target-file ?

        // self.cmd.stdin(Stdio::piped()); // Allow password entry for sudo (local debug)
        self.cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        self.cmd.env_clear();
    }
}
