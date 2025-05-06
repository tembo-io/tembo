use log::error;
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub prometheus_url: String,
    pub prometheus_timeout_ms: i32,
    pub backup_bucket_region: String,
    pub backup_uri_timeout: i32,
    pub temback_image: String,
    pub temback_version: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // The default value is the service name in kubernetes
            prometheus_url: from_env_default(
                "PROMETHEUS_URL",
                "http://monitoring-kube-prometheus-prometheus.monitoring.svc.cluster.local:9090",
            ),

            prometheus_timeout_ms: match from_env_default("PROMETHEUS_TIMEOUT_MS", "500")
                .parse::<i32>()
            {
                Ok(n) => n,
                Err(e) => {
                    error!(
                        "Environment variable PROMETHEUS_TIMEOUT_MS must convert into i32: {}",
                        e
                    );
                    500
                }
            },
            backup_bucket_region: from_env_default("BACKUP_BUCKET_REGION", "us-east-1"),
            backup_uri_timeout: match from_env_default("BACKUP_URI_TIMEOUT", "300").parse::<i32>() {
                Ok(n) => n,
                Err(e) => {
                    error!(
                        "Environment variable BACKUP_URI_TIMEOUT must convert into i32: {}",
                        e
                    );
                    300
                }
            },
            temback_image: from_env_default("TEMBACK_IMAGE", "quay.io/tembo/temback"),
            temback_version: from_env_default("TEMBACK_VERSION", "v0.3.0"),
        }
    }
}

/// source a variable from environment - use default if not exists
fn from_env_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}
