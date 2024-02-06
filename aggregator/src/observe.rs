use std::{collections::HashMap, time::Duration};

use anyhow::Result;
use clap::Args;
use db_model::prefix_tree::PriorityClass;
use lazy_static::lazy_static;
use log::debug;
use opentelemetry::{
    global,
    metrics::{Counter, Gauge, Meter},
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;

lazy_static! {
    static ref METER: Meter = global::meter("prefix-crab.local/crab-tools");
    static ref PREFIXES_AVAILABLE: Gauge<u64> = METER
        .u64_gauge("prefix_crab_schedule_prefixes_available")
        .with_description("Prefixes available for scheduling, regardless of budget")
        .init();
    static ref PREFIXES_ALLOCATED: Gauge<u64> = METER
        .u64_gauge("prefix_crab_schedule_prefixes_allocated")
        .with_description("Prefixes allocated for scheduling after udgeting")
        .init();
    static ref ECHO_ANALYSIS_COUNT: Counter<u64> = METER
        .u64_counter("prefix_crab_echo_analysis_count")
        .with_description("Count of echo analyses")
        .init();
}

#[derive(Args, Clone)]
pub struct Params {
    #[arg(env = "OTLP_ENDPOINT")]
    endpoint: String,

    #[arg(env = "OTLP_AUTH_HEADER")]
    header: String,
}

pub fn initialize(params: Params) -> Result<()> {
    debug!("Sending metrics to {}", params.endpoint);
    let exporter = opentelemetry_otlp::new_exporter()
        .http()
        .with_protocol(opentelemetry_otlp::Protocol::HttpBinary)
        .with_endpoint(params.endpoint)
        .with_headers(HashMap::from([("Authorization".to_owned(), params.header)]));

    opentelemetry_otlp::new_pipeline()
        .metrics(opentelemetry_sdk::runtime::Tokio)
        .with_period(Duration::from_secs(15))
        .with_timeout(Duration::from_secs(5))
        .with_exporter(exporter)
        .build()?; // auto-registers as default

    // It would be nice to shut down the provider, but a) difficult b) this one is stateless either way

    Ok(())
}

pub fn record_budget(prio: PriorityClass, available: u64, allocated: u64) {
    PREFIXES_AVAILABLE.record(available, &[KeyValue::new("class", format!("{:?}", prio))]);
    PREFIXES_ALLOCATED.record(allocated, &[KeyValue::new("class", format!("{:?}", prio))]);
}

pub fn record_echo_analysis(follow_up: bool) {
    ECHO_ANALYSIS_COUNT.add(1, &[KeyValue::new("follow_up", follow_up)])
}
