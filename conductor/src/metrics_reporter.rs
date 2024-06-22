use std::{env, time::Duration};

use anyhow::{bail, Context, Result};
use conductor::metrics::dataplane_metrics::split_data_plane_metrics;
use conductor::metrics::{dataplane_metrics::DataPlaneMetrics, prometheus::Metrics};
use log::info;
use pgmq::PGMQueueExt;
use serde::Deserialize;
use tokio::time::interval;

const METRICS_FILE: &str = include_str!("../metrics.yml");

use crate::from_env_default;

#[derive(Debug, Deserialize)]
pub struct MetricQuery {
    name: String,
    query: String,
}

#[derive(Debug, Deserialize)]
pub struct MetricQueries {
    metrics: Vec<MetricQuery>,
}

fn load_metric_queries() -> Result<MetricQueries> {
    serde_yaml::from_str(METRICS_FILE).map_err(Into::into)
}

pub async fn run_metrics_reporter() -> Result<()> {
    let client = Client::new();

    let MetricQueries { metrics } = load_metric_queries()?;
    info!("metrics_reporter: loaded {} metrics", metrics.len());

    let pg_conn_url = env::var("POSTGRES_QUEUE_CONNECTION")
        .with_context(|| "POSTGRES_QUEUE_CONNECTION must be set")?;

    let queue = PGMQueueExt::new(pg_conn_url, 5).await?;
    let metrics_events_queue =
        env::var("METRICS_EVENTS_QUEUE").expect("METRICS_EVENTS_QUEUE must be set");

    queue.init().await?;
    queue.create(&metrics_events_queue).await?;

    let mut sync_interval = interval(Duration::from_secs(60));

    loop {
        sync_interval.tick().await;

        for metric in &metrics {
            info!("Querying '{}' from Prometheus", metric.name);
            let metrics = client.query(&metric.query).await?;

            let num_metrics = metrics.data.result.len();
            info!(
                "Successfully queried `{}`, num_metrics: `{}` from Prometheus",
                metric.name, num_metrics
            );

            let data_plane_metrics = DataPlaneMetrics {
                name: metric.name.clone(),
                result: metrics.data.result,
            };

            let batch_size = 1000;
            let metrics_to_send = split_data_plane_metrics(data_plane_metrics, batch_size);
            let batches = metrics_to_send.len();

            info!(
                "Split metrics into {} chunks, each with {} results",
                batches, batch_size
            );

            let mut i = 1;
            for data_plane_metrics in &metrics_to_send {
                queue
                    .send(&metrics_events_queue, data_plane_metrics)
                    .await?;
                info!("Enqueued batch {}/{} to PGMQ", i, batches);
                i += 1;
            }
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

    pub async fn query(&self, query: &str) -> Result<Metrics> {
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
