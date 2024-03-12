use std::{collections::HashMap, time::Duration};

use anyhow::Result;
use clap::Args;
use db_model::prefix_tree::{AsNumber, PriorityClass};
use lazy_static::lazy_static;
use log::debug;
use opentelemetry::{
    global,
    metrics::{Counter, Gauge, Meter},
    KeyValue,
};
use opentelemetry_otlp::{HttpExporterBuilder, WithExportConfig};
use opentelemetry_sdk::{runtime::Tokio, trace::config, Resource};
use tracing_subscriber::{filter::filter_fn, layer::SubscriberExt, Layer, Registry};

lazy_static! {
    static ref METER: Meter = global::meter("prefix-crab.local/aggregator");
    static ref PREFIXES_AVAILABLE: Gauge<u64> = METER
        .u64_gauge("prefix_crab_schedule_prefixes_available")
        .with_description("Prefixes available for scheduling, regardless of budget")
        .init();
    static ref PREFIXES_ALLOCATED: Gauge<u64> = METER
        .u64_gauge("prefix_crab_schedule_prefixes_allocated")
        .with_description("Prefixes allocated for scheduling after udgeting")
        .init();
    static ref AS_BUDGET_ALLOCATED: Gauge<u64> = METER
        .u64_gauge("prefix_crab_schedule_asn_allocated")
        .with_description("Prefixes allocated for a single ASN")
        .init();
    static ref ECHO_ANALYSIS_COUNT: Counter<u64> = METER
        .u64_counter("prefix_crab_echo_analysis_count")
        .with_description("Count of echo analyses")
        .init();
    static ref SPLIT_DECISION_COUNT: Counter<u64> = METER
        .u64_counter("prefix_crab_split_decision_count_v2") // v1 missed instance indicators
        .with_description("Count of split decisions")
        .init();
}

#[derive(Args, Clone)]
pub struct Params {
    #[arg(long, env = "OTLP_ENDPOINT", default_value = "")]
    endpoint: String,

    #[arg(long, env = "OTLP_AUTH_HEADER", default_value = "")]
    header: String,

    #[arg(
        long = "otlp-instance",
        env = "OTLP_INSTANCE",
        default_value = "default"
    )]
    instance: String,
}

pub struct ObserveDropGuard {}

pub fn initialize(params: Params) -> Result<Option<ObserveDropGuard>> {
    if params.endpoint.is_empty() {
        return Ok(None);
    }

    debug!("Sending OTLP data to {} as {}", params.endpoint, params.instance);

    let resource = Resource::new(vec![
        KeyValue::new("service.name", "aggregator"),
        KeyValue::new("service.instance.id", params.instance.to_owned()),
    ]);

    opentelemetry_otlp::new_pipeline()
        .metrics(Tokio)
        .with_period(Duration::from_secs(30))
        .with_timeout(Duration::from_secs(5))
        .with_exporter(make_exporter(&params))
        .with_resource(resource.clone())
        .build()?; // auto-registers as default

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(make_exporter(&params))
        .with_trace_config(config().with_resource(resource))
        .install_batch(Tokio)?;

    let telemetry = tracing_opentelemetry::layer()
        .with_tracer(tracer)
        .with_filter(filter_fn(|metadata| {
            metadata.module_path() != Some("isahc::handler") && // Trace exporter is very noisy otherwise
            metadata.module_path() != Some("isahc::agent")
        }));

    let subscriber = Registry::default().with(telemetry);
    tracing::subscriber::set_global_default(subscriber)?;

    Ok(Some(ObserveDropGuard {}))
}

fn make_exporter(params: &Params) -> HttpExporterBuilder {
    opentelemetry_otlp::new_exporter()
        .http()
        .with_protocol(opentelemetry_otlp::Protocol::HttpBinary)
        .with_endpoint(&params.endpoint)
        .with_headers(HashMap::from([(
            "Authorization".to_owned(),
            params.header.to_owned(),
        )]))
}

impl Drop for ObserveDropGuard {
    fn drop(&mut self) {
        debug!("Shutting down tracer");
        global::shutdown_tracer_provider(); // Must happen outside of Tokio runtime, otherwise blocks forever
                                            // OTLP metrics exporter doesn't need shutdown
    }
}

pub fn record_budget(prio: PriorityClass, available: u64, allocated: u64) {
    PREFIXES_AVAILABLE.record(available, &[KeyValue::new("class", format!("{:?}", prio))]);
    PREFIXES_ALLOCATED.record(allocated, &[KeyValue::new("class", format!("{:?}", prio))]);
}

pub fn record_as_budget_usage(asn: AsNumber, consumed: u64) {
    AS_BUDGET_ALLOCATED.record(consumed, &[KeyValue::new("asn", format!("{:?}", asn))]);
}

pub fn record_echo_analysis(follow_up: bool) {
    ECHO_ANALYSIS_COUNT.add(1, &[KeyValue::new("follow_up", follow_up)])
}

pub fn record_split_decision(prio: PriorityClass, action_performed: bool, would_split: bool) {
    SPLIT_DECISION_COUNT.add(
        1,
        &[
            KeyValue::new("class", format!("{:?}", prio)),
            KeyValue::new("performed", action_performed),
            KeyValue::new("would_split", would_split),
        ],
    )
}
