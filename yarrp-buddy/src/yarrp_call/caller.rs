use std::borrow::Cow;
use std::io::{self, BufRead, Read};
use std::net::Ipv6Addr;
use std::process::{ChildStdout, Command, Stdio};

use crate::schedule::ProbeResponse;
use anyhow::{bail, Context, Result};
use log::Level::Debug;
use log::{debug, error, log_enabled, trace, warn};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use super::targets::TargetCollector;
use super::Params;

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

    /// Request responses to be captured instead of just printed.
    /// Responses will be provided to the returned [Receiver].
    /// The sender will be dropped once zmap closes stdout (i.e. exits).
    pub fn request_responses(&mut self) -> UnboundedReceiver<ProbeResponse> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.response_tx = Some(tx);
        rx
    }

    pub fn setup(&mut self, params: &Params) {
        if let Some(interface_name) = &params.interface {
            self.cmd.arg(format!("--interface={}", interface_name));
        }
        if let Some(gateway_mac) = &params.gateway_mac {
            self.cmd.arg(format!("--dstmac={}", gateway_mac));
        }
        params
            .source_address
            .parse::<Ipv6Addr>()
            .expect("IPv6 source address to be a valid IPv6 address");
        self.cmd
            .arg(format!("--srcaddr={}", params.source_address))
            .arg(format!("--rate={}", params.yarrp_rate_pps))
            .arg(format!("--minttl={}", params.min_ttl))
            .arg(format!("--maxttl={}", params.max_ttl))
            .arg(format!("--neighborhood={}", params.neighborhood_max_ttl))
            .arg(format!("--fillmode={}", params.fill_mode_max_ttl))
            .arg("--shutdown-wait=10")
            .arg("--max_null_reads=5");
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
        self.cmd.arg(format!("--input={}", path_as_str));
        Ok(())
    }

    fn do_call(&mut self) -> Result<()> {
        if log_enabled!(Debug) {
            let args: Vec<Cow<'_, str>> = self
                .cmd
                .get_args()
                .map(|os_str| os_str.to_string_lossy())
                .collect();
            debug!("Calling yarrp with arguments: {}", args.join(" "));
        }

        let mut child = self
            .cmd
            .spawn()
            .with_context(|| "Failed to spawn yarrp process")?;

        self.watch_logger_fd(&mut child.stderr);
        match self.response_tx.take() {
            Some(tx) => self.watch_stdout(&mut child.stdout, tx),
            None => self.watch_logger_fd(&mut child.stdout),
        }

        let exit_status = child
            .wait()
            .with_context(|| "Failed to wait for child to exit")?;

        if exit_status.success() {
            debug!("yarrp call exited successfully");
            Ok(())
        } else {
            bail!(
                "yarrp call exited with non-successful status {:?}",
                exit_status
            )
        }
    }

    fn watch_logger_fd<R: Read + Send + 'static>(&self, fd: &mut Option<R>) {
        let taken = fd.take().expect("Failed to open output stream of child");
        std::thread::spawn(move || {
            let reader = io::BufReader::new(taken);
            for line in reader.lines().flatten() {
                if line.starts_with("*** Fatal") {
                    error!("yarrp: {}", line);
                } else {
                    trace!("yarrp: {}", line);
                }
            }
        });
    }

    fn watch_stdout(&mut self, fd: &mut Option<ChildStdout>, tx: UnboundedSender<ProbeResponse>) {
        let taken_fd = fd.take().expect("Failed to open output stream of child");
        std::thread::spawn(move || {
            let mut reader = csv::ReaderBuilder::new()
                .comment(Some(b'#'))
                .delimiter(b' ')
                .has_headers(false)
                .flexible(true) // needed because non-csv data is written to stdout
                .from_reader(taken_fd);
            let fake_header = csv::ByteRecord::from(vec![
                "intended_target",
                "ts_sec",
                "ts_usec",
                "icmp_type",
                "icmp_code",
                "sent_ttl",
                "actual_from",
                "roundtrip_time",
                "ipid_v4_only",
                "sent_packet_size",
                "recv_packet_size",
                "received_ttl",
                "rtos_idk",
                "mpls_label",
                "mystery_counter",
            ]);
            let mut work_record = csv::ByteRecord::new();
            loop {
                let res = reader.read_byte_record(&mut work_record);
                if let Err(e) = res {
                    warn!("Failed to read record from yarrp: {}", e);
                    continue;
                } else if !res.unwrap() {
                    break; // EOF
                }
                trace!("[[yarrp record]] {:?}", work_record);
                if work_record.get(0).map(|it| it == b">>").unwrap_or(false) {
                    continue;
                }
                let model_res = work_record.deserialize(Some(&fake_header));
                if let Err(e) = model_res {
                    warn!("Failed to deserialise record: {:?} - from: {:?}", e, work_record);
                    continue;
                }
                if let Err(e) = tx.send(model_res.unwrap()) {
                    warn!(
                        "Unable to send response over channel; maybe the receiver disconnected? {}",
                        e
                    );
                    break;
                }
            }
            trace!("Done reading from yarrp stdout");
            drop(tx);
        });
    }

    fn set_base(&mut self) {
        self.cmd.arg("--type=ICMP6").arg("--output=-");

        self.cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        self.cmd.env_clear();
    }
}
