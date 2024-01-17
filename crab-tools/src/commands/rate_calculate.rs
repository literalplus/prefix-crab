use anyhow::Result;
use clap::Args;

#[derive(Args, Clone)]
pub struct Params {
    #[clap(long, default_value = "120")]
    schedule_interval_seconds: f64,

    #[clap(long, default_value = "75")]
    zmap_pps: f64,

    #[clap(long, default_value = "500000")]
    zmap_target_rate_total: f64,
}

const ZMAP_PACKET_BITS: f64 = 560.0;
const ZMAP_PACKETS_PER_PREFIX: f64 = 32.0;
const ZMAP_SHUTDOWN_WAIT: f64 = 23.0; // for NDP timeout

const YARRP_PACKET_BITS: f64 = 720.0;
const YARRP_HOPS_PER_PROBE: f64 = 15.0;
const YARRP_PACKETS_PER_PREFIX_AVG: f64 = (32.0 / 2.0) * YARRP_HOPS_PER_PROBE;
const YARRP_SHUTDOWN_WAIT: f64 = 10.0;

pub fn handle(params: Params) -> Result<()> {
    let zmap_as_data_rate = ZMAP_PACKET_BITS * params.zmap_pps;
    let zmap_bits_per_prefix = ZMAP_PACKET_BITS * ZMAP_PACKETS_PER_PREFIX;
    let zmap_prefixes_per_second = zmap_as_data_rate / zmap_bits_per_prefix;
    let zmap_seconds_per_schedule = params.schedule_interval_seconds - ZMAP_SHUTDOWN_WAIT;
    let as_prefixes_per_period = zmap_seconds_per_schedule * zmap_prefixes_per_second;

    println!("[ZMAP/pfx] target rate: {} pps", params.zmap_pps);
    println!("[ZMAP/pfx] prefix rate: {} pfx/s", zmap_prefixes_per_second);
    println!(
        "[ZMAP/pfx] prefixes per period: {} pfx/1",
        as_prefixes_per_period
    );
    println!(
        "[ZMAP/pfx] data rate: {} bit/s\n",
        zmap_prefixes_per_second * zmap_bits_per_prefix
    );

    let prefixes_in_target_rate =
        params.zmap_target_rate_total / (zmap_bits_per_prefix * zmap_prefixes_per_second);
    let prefixes_target = prefixes_in_target_rate * as_prefixes_per_period;

    println!(
        "[ZMAP/total] target data rate: {} bit/s",
        params.zmap_target_rate_total
    );
    println!(
        "[ZMAP/total] prefixes per second: {} pfx/s",
        prefixes_in_target_rate
    );
    println!(
        "[ZMAP/total] prefixes per schedule: {} pfx/1",
        prefixes_target
    );
    println!(
        "[ZMAP/total] pps: {}\n",
        params.zmap_target_rate_total / ZMAP_PACKET_BITS
    );

    let yarrp_packets_per_prefix = YARRP_PACKET_BITS * YARRP_PACKETS_PER_PREFIX_AVG;
    let yarrp_seconds_per_schedule = params.schedule_interval_seconds - YARRP_SHUTDOWN_WAIT;
    let yarrp_prefixes_per_second = prefixes_target / yarrp_seconds_per_schedule;
    let yarrp_pps = yarrp_prefixes_per_second * YARRP_PACKETS_PER_PREFIX_AVG;
    let yarrp_rate = yarrp_pps * YARRP_PACKET_BITS;

    println!(
        "[yarrp/total] packets per prefix: {}",
        yarrp_packets_per_prefix
    );
    println!("[yarrp/total] pps: {}", yarrp_pps);
    println!("[yarrp/total] data rate: {} bit/s", yarrp_rate);

    Ok(())
}
