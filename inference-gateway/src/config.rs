use std::collections::HashMap;
use std::env;

use url::Url;

#[derive(Clone, Debug)]
pub struct Config {
    pub model_service_map: HashMap<String, Url>,
    /// Postgres connection string to the timeseries database which logs token usage
    pub pg_conn_str: String,
    /// Postgres connection string for the Control Plane queue
    pub billing_queue_conn_str: String,
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
            model_service_map: parse_model_service_port_map(),
            pg_conn_str: from_env_default(
                "DATABASE_URL",
                "postgresql://postgres:postgres@0.0.0.0:5432/postgres",
            ),
            billing_queue_conn_str: from_env_default(
                "QUEUE_CONN_URL",
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

/// MODEL_NAME_SERVICE_PORT_MAP -- a comma separate list of model names and the host:port they are served at
/// <model-name>=<host>:<port>,<model-name>=<host>:<port>
/// e.g. meta-llama/Meta-Llama-3-8B-Instruct=llama-3-8b-instruct:8000,meta-llama/Llama-3.1-8B-Instruct=llama-3-1-8b-instruct:8000,
/// Must be an OpenAI compatible interface
fn parse_model_service_port_map() -> HashMap<String, Url> {
    let model_mappings_values = from_env_default(
        "MODEL_SERVICE_PORT_MAP",
        "facebook/opt-125m=http://vllm:8000",
    );

    // Initialize an empty HashMap to store model-service-port mappings
    let mut model_map: HashMap<String, Url> = HashMap::new();

    // Split the environment variable value by semicolon to get individual mappings
    for mapping in model_mappings_values.split(',') {
        // Split each mapping into <model_name>=<service>:<port>
        if let Some((model_name, service_port)) = mapping.split_once('=') {
            let svc_port_url = Url::parse(service_port)
                .unwrap_or_else(|_| panic!("malformed service: {service_port}"));
            model_map.insert(model_name.to_string(), svc_port_url);
        }
    }
    model_map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_values() {
        env::remove_var("MODEL_SERVICE_PORT_MAP");

        let result = parse_model_service_port_map();
        let mut expected = HashMap::new();
        expected.insert(
            "facebook/opt-125m".to_string(),
            Url::parse("http://vllm:8000").unwrap(),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_custom_mapping() {
        env::set_var("MODEL_SERVICE_PORT_MAP", "meta-llama/Meta-Llama-3-8B-Instruct=http://tembo-ai-dev-llama-3-8b-instruct.svc.cluster.local:8000");

        let result = parse_model_service_port_map();
        let mut expected = HashMap::new();
        expected.insert(
            "meta-llama/Meta-Llama-3-8B-Instruct".to_string(),
            Url::parse("http://tembo-ai-dev-llama-3-8b-instruct.svc.cluster.local:8000").unwrap(),
        );

        assert_eq!(result, expected);
    }

    #[test]
    #[should_panic(expected = "malformed service: http://vllm:invalid_port")]
    fn test_malformed_url() {
        env::set_var(
            "MODEL_SERVICE_PORT_MAP",
            "facebook/opt-125m=http://vllm:invalid_port",
        );
        parse_model_service_port_map();
    }
}
