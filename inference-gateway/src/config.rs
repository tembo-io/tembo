use std::env;

use url::Url;

#[derive(Clone, Debug)]
pub struct Config {
    /// service and port of the inference service
    /// Must be an OpenAI compatible interface
    pub llm_service_host_port: Url,
    /// Postgres connection string to the timeseries databse which logs token usage
    pub pg_conn_str: String,
    /// Port to run the inference gateway on
    pub server_port: u16,
    /// Number of actix workers to spawn
    pub server_workers: u16,
    /// Boolean to toggle billing request authorization.
    /// When true, callers must have an active payment method on file
    pub org_auth_enabled: bool,
    /// Interval to refresh the billing authorization cache
    pub org_auth_cache_refresh_interval_sec: u64,
    pub run_billing_reporter: bool,
}

impl Config {
    pub async fn new() -> Self {
        Self {
            llm_service_host_port: parse_llm_service(),
            pg_conn_str: from_env_default(
                "DATABASE_URL",
                "postgresql://postgres:postgres@0.0.0.0:5432/postgres",
            ),
            server_port: from_env_default("WEBSERVER_PORT", "8080")
                .parse::<u16>()
                .unwrap_or(8080),
            server_workers: from_env_default("WEBSERVER_WORKERS", "8")
                .parse::<u16>()
                .unwrap_or(8),
            org_auth_enabled: from_env_default("ORG_AUTH_ENABLED", "false")
                .parse()
                .expect("ORG_AUTH_ENABLED must be a boolean"),
            org_auth_cache_refresh_interval_sec: from_env_default(
                "ORG_AUTH_CACHE_REFRESH_INTERVAL_SEC",
                "10",
            )
            .parse()
            .expect("ORG_AUTH_CACHE_REFRESH_INTERVAL_SEC must be an integer"),
            run_billing_reporter: from_env_default("RUN_BILLING_REPORTER", "false")
                .parse()
                .unwrap(),
        }
    }
}

/// source a variable from environment - use default if not exists
fn from_env_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn parse_llm_service() -> Url {
    let value = from_env_default("LLM_SERVICE_HOST_PORT", "http://vllm:8000");
    Url::parse(&value).unwrap_or_else(|_| panic!("malformed LLM_SERVICE_HOST_PORT: {value}"))
}
