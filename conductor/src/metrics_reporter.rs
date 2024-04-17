use std::{env, fs::File, time::Duration};

use anyhow::{bail, Context, Result};
use log::info;
use serde::Deserialize;
use serde_json::Value;
use tokio::time::interval;

const METRICS_FILE: &str = include_str!("../metrics.yml");

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
    serde_yaml::from_str(METRICS_FILE).with_context(|| "Failed to deserialize METRICS_FILE")
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
    query_url: String,
    client: reqwest::Client,
}

impl Client {
    pub fn new() -> Self {
        let prometheus_url = from_env_default(
            "PROMETHEUS_URL",
            "http://monitoring-kube-prometheus-prometheus.monitoring.svc.cluster.local:9090",
        );
        let query_url = format!("{prometheus_url}/api/v1/query");

        info!("metrics_reporter will use '{query_url}'");

        Self {
            query_url,
            client: reqwest::Client::new(),
        }
    }

    pub async fn query(&self, query: &str) -> Result<Value> {
        let response = self
            .client
            .get(&self.query_url)
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
