use std::{collections::HashMap, time::Duration};

use anyhow::Result;
use clap::Args;
use futures::executor;
use lazy_static::lazy_static;
use opentelemetry::{
    global,
    metrics::{Counter, Meter},
    KeyValue,
};
use opentelemetry_otlp::{HttpExporterBuilder, WithExportConfig};
use opentelemetry_sdk::{runtime::Tokio, trace, Resource};
use tracing::instrument;
use tracing_subscriber::{filter::filter_fn, layer::SubscriberExt, Layer, Registry};

lazy_static! {
    static ref METER: Meter = global::meter("prefix-crab.local/crab-tools");
    static ref COUNTER: Counter<u64> = METER
        .u64_counter("example_counter")
        .with_description("example")
        .init();
}

#[derive(Args, Clone)]
pub struct Params {
    #[arg(env = "OTLP_ENDPOINT")]
    endpoint: String,

    #[arg(env = "OTLP_AUTH_HEADER")]
    headers: String,
}

pub fn handle(params: Params) -> Result<()> {
    let handle = tokio::spawn(run(params));

    executor::block_on(handle)??;

    global::shutdown_tracer_provider(); // must be called outside of Tokio, otherwise blocks forever

    Ok(())
}

async fn run(params: Params) -> Result<()> {
    println!("Sending metrics to {}", params.endpoint);
    opentelemetry_otlp::new_pipeline()
        .metrics(Tokio)
        .with_period(Duration::from_secs(5))
        .with_timeout(Duration::from_secs(3))
        .with_exporter(make_exporter(&params))
        .build()?;

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(make_exporter(&params))
        .with_trace_config(
            trace::config().with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                "crab-tools",
            )])),
        )
        .install_batch(Tokio)?;

    let telemetry = tracing_opentelemetry::layer()
        .with_tracer(tracer)
        .with_filter(filter_fn(|metadata| {
            metadata.module_path() != Some("isahc::handler")
        }));
    let subscriber = Registry::default().with(telemetry);

    tracing::subscriber::set_global_default(subscriber)?;

    my_test_fun("hello its me").await;

    COUNTER.add(2, &[]);

    Ok(())
}

fn make_exporter(params: &Params) -> HttpExporterBuilder {
    opentelemetry_otlp::new_exporter()
        .http()
        .with_protocol(opentelemetry_otlp::Protocol::HttpBinary)
        .with_endpoint(&params.endpoint)
        .with_headers(HashMap::from([(
            "Authorization".to_owned(),
            params.headers.to_owned(),
        )]))
}

#[instrument]
async fn my_test_fun(param: &str) {
    sub_fun(param);
}

#[instrument]
fn sub_fun(param: &str) {
    tracing::warn!("oh no! a {}", param)
}
