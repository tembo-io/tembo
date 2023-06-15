use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub prometheus_url: String,
    pub prometheus_timeout_ms: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // The default value is the service name in kubernetes
            prometheus_url: from_env_default(
                "PROMETHEUS_URL",
                "http://monitoring-kube-prometheus-prometheus.monitoring.svc.cluster.local:9090",
            ),
            prometheus_timeout_ms: match from_env_default("PROMETHEUS_TIMEOUT_MS", "100")
                .parse::<i32>()
            {
                Ok(n) => n,
                Err(_e) => 100,
            },
        }
    }
}

/// source a variable from environment - use default if not exists
fn from_env_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}
