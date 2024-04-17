use std::{env, fs::File, time::Duration};

use anyhow::{bail, Context, Result};
use log::info;
use pgmq::query;
use serde::Deserialize;
use serde_json::Value;
use tokio::time::interval;

use crate::from_env_default;

#[derive(Debug, Deserialize)]
pub struct Metric {
    name: String,
    query: String,
}

#[derive(Debug, Deserialize)]
pub struct Metrics {
    metrics: Vec<Metric>,
}

fn load_metrics() -> Result<Metrics> {
    let metrics_filename =
        env::var("METRICS_FILE").with_context(|| "METRICS_FILE env var not found!")?;
    let metrics = File::open(metrics_filename).with_context(|| "Failed to open METRICS_FILE")?;

    serde_yaml::from_reader(metrics).with_context(|| "Failed to deserialize METRICS_FILE")
}

pub async fn run_metrics_reporter() -> Result<()> {
    let client = Client::new();
    let Metrics { metrics } = load_metrics()?;
    info!("metrics_reporter: loaded {} metrics", metrics.len());
    let mut sync_interval = interval(Duration::from_secs(5));

    loop {
        sync_interval.tick().await;

        for metric in &metrics {
            let resp = client.query(&metric.query).await?;

            println!(
                "Got response: {}",
                serde_json::to_string_pretty(&resp).unwrap()
            )
        }
    }
}

struct Client {
    prometheus_url: String,
    client: reqwest::Client,
}

impl Client {
    pub fn new() -> Self {
        Self {
            prometheus_url: from_env_default(
                "PROMETHEUS_URL",
                "http://monitoring-kube-prometheus-prometheus.monitoring.svc.cluster.local:9090",
            ),
            client: reqwest::Client::new(),
        }
    }

    pub async fn query(&self, query: &str) -> Result<Value> {
        let response = self
            .client
            .get(&self.prometheus_url)
            .query(&[("query", query)])
            .send()
            .await?;

        if response.status().is_success() {
            response.json().await.map_err(Into::into)
        } else {
            let error_msg = response.text().await?;

            bail!("Failed to query Prometheus: {error_msg}")
        }
    }
}
