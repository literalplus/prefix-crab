use std::{collections::HashMap, time::Duration};

use anyhow::Result;
use clap::Args;
use futures::executor;
use lazy_static::lazy_static;
use opentelemetry::{
    global,
    metrics::{Counter, Meter},
};
use opentelemetry_otlp::WithExportConfig;
use tokio::time;

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

    Ok(())
}

async fn run(params: Params) -> Result<()> {
    println!("Sending metrics to {}", params.endpoint);
    let exporter = opentelemetry_otlp::new_exporter()
        .http()
        .with_protocol(opentelemetry_otlp::Protocol::HttpBinary)
        .with_endpoint(params.endpoint)
        .with_headers(HashMap::from([(
            "Authorization".to_owned(),
            params.headers,
        )]));

    let meter_provider = opentelemetry_otlp::new_pipeline()
        .metrics(opentelemetry_sdk::runtime::Tokio)
        .with_period(Duration::from_secs(5))
        .with_timeout(Duration::from_secs(3))
        .with_exporter(exporter)
        .build()?;
    COUNTER.add(2, &[]);
    time::sleep(Duration::from_secs(30)).await;

    println!("Shutdown result: {:?}", meter_provider.shutdown()); // https://github.com/open-telemetry/opentelemetry-rust/pull/1375

    Ok(())
}
