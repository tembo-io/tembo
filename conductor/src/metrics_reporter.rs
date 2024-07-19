use anyhow::{bail, Context, Result};
use conductor::metrics::dataplane_metrics::split_data_plane_metrics;
use conductor::metrics::{dataplane_metrics::DataPlaneMetrics, prometheus::Metrics};
use log::info;
use pgmq::PGMQueueExt;
use serde::Deserialize;
use std::time::Instant;
use std::{env, time::Duration};
use tokio::time::interval;

const METRICS_FILE: &str = include_str!("../metrics.yml");

use crate::from_env_default;

#[derive(Debug, Deserialize)]
pub struct MetricQuery {
    name: String,
    server: ServerType,
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
    let client = Client::new().await;

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

        let now = Instant::now();
        for metric in &metrics {
            info!("Querying '{}' from {}", metric.name, metric.server);

            let metrics = client.query(&metric.query, &metric.server).await?;

            let num_metrics = metrics.data.result.len();
            info!(
                "Successfully queried `{}`, num_metrics: `{}` from {}",
                metric.name, num_metrics, metric.server
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
            info!("Processed metric in {:?}", now.elapsed());
        }
    }
}

struct Client {
    prometheus_url: String,
    loki_url: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ServerType {
    Prometheus,
    Loki,
}

impl std::fmt::Display for ServerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerType::Prometheus => write!(f, "Prometheus"),
            ServerType::Loki => write!(f, "Loki"),
        }
    }
}

impl Client {
    pub async fn new() -> Self {
        let prometheus_url = from_env_default(
            "PROMETHEUS_URL",
            "http://monitoring-kube-prometheus-prometheus.monitoring.svc.cluster.local:9090/api/v1/query",
        );
        let loki_url = from_env_default(
            "LOKI_URL",
            "http://loki-gateway.monitoring.svc.cluster.local/loki/api/v1/query",
        );

        info!("metrics_reporter will use '{prometheus_url}' for Prometheus");
        info!("metrics_reporter will use '{loki_url}' for Loki");

        Self {
            prometheus_url,
            loki_url,
            client: reqwest::Client::new(),
        }
    }

    pub async fn query(&self, query: &str, server_type: &ServerType) -> Result<Metrics> {
        let query_url = match server_type {
            ServerType::Prometheus => &self.prometheus_url,
            ServerType::Loki => &self.loki_url,
        };

        let mut request = self.client.get(query_url).query(&[("query", query)]);

        if matches!(server_type, ServerType::Loki) {
            request = request.header("X-Scope-OrgID", "internal");
        }

        let response = request.send().await?;

        if response.status().is_success() {
            response.json().await.map_err(Into::into)
        } else {
            let error_msg = response.text().await?;
            bail!("Failed to query {}: {error_msg}", server_type)
        }
    }
}
