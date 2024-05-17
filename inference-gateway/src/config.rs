use std::env;

use url::Url;

#[derive(Clone, Debug)]
pub struct Config {
    pub llm_service_host_port: Url,
    pub pg_conn_str: String,
    pub server_port: u16,
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
