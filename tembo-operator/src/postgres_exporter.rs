use crate::{apis::coredb_types::CoreDB, defaults, Error};
use kube::Client;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tracing::debug;

pub const QUERIES: &str = "tembo-queries";
pub const EXPORTER_VOLUME: &str = "postgres-exporter";
pub const EXPORTER_CONFIGMAP_PREFIX: &str = "metrics-";

/// PostgresExporter is the configuration for the postgres-exporter to expose
/// custom metrics from the database.
///
/// **Example:** This example exposes specific metrics from a query to a
/// [pgmq](https://github.com/tembo-io/pgmq) queue enabled database.
///
/// ```yaml
/// apiVersion: coredb.io/v1alpha1
/// kind: CoreDB
/// metadata:
///   name: test-db
/// spec:
/// metrics:
///   enabled: true
///   image: quay.io/prometheuscommunity/postgres-exporter:v0.12.0
///   queries:
///     pgmq:
///       query: select queue_name, queue_length, oldest_msg_age_sec, newest_msg_age_sec, total_messages from public.pgmq_metrics_all()
///       master: true
///       metrics:
///         - queue_name:
///             description: Name of the queue
///             usage: LABEL
///         - queue_length:
///             description: Number of messages in the queue
///             usage: GAUGE
///         - oldest_msg_age_sec:
///             description: Age of the oldest message in the queue, in seconds.
///             usage: GAUGE
///         - newest_msg_age_sec:
///             description: Age of the newest message in the queue, in seconds.
///             usage: GAUGE
///         - total_messages:
///             description: Total number of messages that have passed into the queue.
///             usage: GAUGE
/// ````
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
#[allow(non_snake_case)]
pub struct PostgresMetrics {
    /// The image to use for the postgres-exporter container.
    ///
    /// **Default:** `quay.io/prometheuscommunity/postgres-exporter:v0.12.0`
    #[serde(default = "defaults::default_postgres_exporter_image")]
    pub image: String,

    /// To enable or disable the metric.
    ///
    /// **Default:** `true`
    #[serde(default = "defaults::default_postgres_exporter_enabled")]
    pub enabled: bool,

    /// The SQL query to run.
    ///
    /// **Example:** `select queue_name, queue_length, oldest_msg_age_sec, newest_msg_age_sec, total_messages from public.pgmq_metrics_all()`
    ///
    /// **Default**: `None`
    #[schemars(schema_with = "preserve_arbitrary")]
    pub queries: Option<QueryConfig>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct Metric {
    pub usage: Usage,
    pub description: String,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    #[serde(flatten)]
    pub metrics: BTreeMap<String, Metric>,
}

/// **Example**: This example exposes specific metrics from a query to a
/// [pgmq](https://github.com/tembo-io/pgmq) queue enabled database.
///
/// ```yaml
///   metrics:
///    enabled: true
///    image: quay.io/prometheuscommunity/postgres-exporter:v0.12.0
///    queries:
///      pgmq:
///        query: select queue_name, queue_length, oldest_msg_age_sec, newest_msg_age_sec, total_messages from pgmq.metrics_all()
///        primary: true
///        metrics:
///          - queue_name:
///              description: Name of the queue
///              usage: LABEL
///          - queue_length:
///              description: Number of messages in the queue
///              usage: GAUGE
///          - oldest_msg_age_sec:
///              description: Age of the oldest message in the queue, in seconds.
///              usage: GAUGE
///          - newest_msg_age_sec:
///              description: Age of the newest message in the queue, in seconds.
///              usage: GAUGE
///          - total_messages:
///              description: Total number of messages that have passed into the queue.
///              usage: GAUGE
///        target_databases:
///          - "postgres"
/// ```
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct QueryItem {
    /// the SQL query to run on the target database to generate the metrics
    pub query: String,

    // We need to support this at some point going forward since master
    // is now deprecated.
    // whether to run the query only on the primary instance
    //pub primary: Option<bool>,

    // same as primary (for compatibility with the Prometheus PostgreSQL
    // exporter's syntax - **deprecated**)
    /// whether to run the query only on the master instance
    /// See [https://cloudnative-pg.io/documentation/1.20/monitoring/#structure-of-a-user-defined-metric](https://cloudnative-pg.io/documentation/1.20/monitoring/#structure-of-a-user-defined-metric)
    pub master: bool,

    /// the name of the column returned by the query
    ///
    /// usage: one of the values described below
    /// description: the metric's description
    /// metrics_mapping: the optional column mapping when usage is set to MAPPEDMETRIC
    pub metrics: Vec<Metrics>,

    /// The default database can always be overridden for a given user-defined
    /// metric, by specifying a list of one or more databases in the target_databases
    /// option.
    ///
    /// See: [https://cloudnative-pg.io/documentation/1.20/monitoring/#example-of-a-user-defined-metric-running-on-multiple-databases](https://cloudnative-pg.io/documentation/1.20/monitoring/#example-of-a-user-defined-metric-running-on-multiple-databases)
    ///
    /// **Default:** `["postgres"]`
    #[serde(default = "defaults::default_postgres_exporter_target_databases")]
    pub target_databases: Vec<String>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct QueryConfig {
    #[serde(flatten)]
    pub queries: BTreeMap<String, QueryItem>,
}

// source: https://github.com/kube-rs/kube/issues/844
fn preserve_arbitrary(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
    let mut obj = schemars::schema::SchemaObject::default();
    obj.extensions
        .insert("x-kubernetes-preserve-unknown-fields".into(), true.into());
    schemars::schema::Schema::Object(obj)
}

use crate::configmap::apply_configmap;

use std::str::FromStr;

#[derive(Clone, Debug, JsonSchema, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Usage {
    Counter,
    Gauge,
    Histogram,
    Label,
}

impl FromStr for Usage {
    type Err = ();

    fn from_str(input: &str) -> Result<Usage, Self::Err> {
        match input {
            "COUNTER" => Ok(Usage::Counter),
            "GAUGE" => Ok(Usage::Gauge),
            "HISTOGRAM" => Ok(Usage::Histogram),
            "LABEL" => Ok(Usage::Label),
            _ => Err(()),
        }
    }
}

pub async fn reconcile_metrics_configmap(cdb: &CoreDB, client: Client, ns: &str) -> Result<(), Error> {
    // set custom pg-prom metrics in configmap values if they are specified
    let coredb_name = cdb
        .metadata
        .name
        .clone()
        .expect("instance should always have a name");
    // Make sure we always check for queries in the spec, incase someone calls this function
    // directly and not through the reconcile function.
    match cdb.spec.metrics.clone().and_then(|m| m.queries) {
        Some(queries) => {
            let qdata = serde_yaml::to_string(&queries)?;
            let d: BTreeMap<String, String> = BTreeMap::from([(QUERIES.to_string(), qdata)]);
            apply_configmap(
                client.clone(),
                ns,
                &format!("{}{}", EXPORTER_CONFIGMAP_PREFIX, coredb_name),
                d,
            )
            .await?
        }
        None => {
            debug!("No queries specified in CoreDB spec {}", coredb_name);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml;

    #[test]
    fn query_deserialize_serialize() {
        // query data received as json. map to struct.
        // serialize struct to yaml
        let incoming_data = serde_json::json!(
            {
                "pg_postmaster": {
                  "query": "SELECT pg_postmaster_start_time as start_time_seconds from pg_postmaster_start_time()",
                  "master": true,
                  "metrics": [
                    {
                      "start_time_seconds": {
                        "usage": "GAUGE",
                        "description": "Time at which postmaster started"
                      }
                    }
                  ]
                },
                "extensions": {
                  "query": "select count(*) as num_ext from pg_available_extensions",
                  "master": true,
                  "metrics": [
                    {
                      "num_ext": {
                        "usage": "GAUGE",
                        "description": "Num extensions"
                      }
                    }
                  ]
                }
              }
        );

        let query_config: QueryConfig = serde_json::from_value(incoming_data).expect("failed to deserialize");

        assert!(query_config.queries.contains_key("pg_postmaster"));
        assert!(query_config.queries.contains_key("extensions"));

        let pg_postmaster = query_config.queries.get("pg_postmaster").unwrap();
        assert_eq!(
            pg_postmaster.query,
            "SELECT pg_postmaster_start_time as start_time_seconds from pg_postmaster_start_time()"
        );
        assert!(pg_postmaster.master);
        assert!(pg_postmaster.metrics[0]
            .metrics
            .contains_key("start_time_seconds"));

        let start_time_seconds_metric = pg_postmaster.metrics[0]
            .metrics
            .get("start_time_seconds")
            .unwrap();
        assert_eq!(
            start_time_seconds_metric.description,
            "Time at which postmaster started"
        );

        let extensions = query_config
            .queries
            .get("extensions")
            .expect("extensions not found");
        assert_eq!(
            extensions.query,
            "select count(*) as num_ext from pg_available_extensions"
        );
        assert!(extensions.master);
        assert!(extensions.metrics[0].metrics.contains_key("num_ext"));

        // yaml to yaml

        let yaml = serde_yaml::to_string(&query_config).expect("failed to serialize to yaml");

        let data = r#"extensions:
  query: select count(*) as num_ext from pg_available_extensions
  master: true
  metrics:
  - num_ext:
      usage: GAUGE
      description: Num extensions
pg_postmaster:
  query: SELECT pg_postmaster_start_time as start_time_seconds from pg_postmaster_start_time()
  master: true
  metrics:
  - start_time_seconds:
      usage: GAUGE
      description: Time at which postmaster started
"#;
        // formmatted correctly as yaml (for configmap)
        assert_eq!(yaml, data);
    }
}
