use std::collections::HashMap;
use std::env;

use url::Url;

use crate::errors::PlatformError;

#[derive(Clone, Debug)]
pub struct Config {
    pub model_rewrites: HashMap<String, String>,
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
            model_rewrites: parse_model_rewrite(),
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

fn parse_model_rewrite() -> HashMap<String, String> {
    let mut map = HashMap::new();

    if let Ok(env_var) = env::var("MODEL_REWRITES") {
        for pair in env_var.split(',') {
            if let Some((key, value)) = pair.split_once(':') {
                map.insert(key.to_string(), value.to_string());
            }
        }
    }

    map
}

#[derive(Debug)]
pub struct MappedRequest {
    // the mapped model name
    pub model: String,
    // url to the correct service for the model
    pub base_url: Url,
    // request body with updated model name
    pub body: serde_json::Value,
}

pub fn rewrite_model_request(
    mut body: serde_json::Value,
    config: &Config,
) -> Result<MappedRequest, PlatformError> {
    // map the model, if there is a mapping for it
    let target_model = if let Some(model) = body.get("model") {
        let requested_model = model.as_str().ok_or_else(|| {
            PlatformError::InvalidQuery("empty value in `model` parameter".to_string())
        })?;

        if let Some(rewritten_model) = config.model_rewrites.get(requested_model) {
            body["model"] = serde_json::Value::String(rewritten_model.clone());
            rewritten_model
        } else {
            requested_model
        }
    } else {
        Err(PlatformError::InvalidQuery(
            "missing `model` parameter in request body".to_string(),
        ))?
    };

    let base_url = config
        .model_service_map
        .get(target_model)
        .ok_or_else(|| PlatformError::InvalidQuery(format!("model {} not found", target_model)))?
        .clone();

    Ok(MappedRequest {
        model: target_model.to_string(),
        base_url,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_rewrite() {
        env::set_var("MODEL_REWRITES", "cat:dog,old:young");
        env::set_var(
            "MODEL_SERVICE_PORT_MAP",
            "dog=http://dog:8000/,young=http://young:8000/",
        );

        let cfg = Config::new().await;
        let body = serde_json::json!({
            "model": "cat",
            "key": "value"
        });

        let rewritten = rewrite_model_request(body.clone(), &cfg).unwrap();
        assert_eq!(rewritten.model, "dog");
        assert_eq!(rewritten.base_url.to_string(), "http://dog:8000/");
        assert_eq!(rewritten.body.get("key").unwrap(), "value");

        let body = serde_json::json!({
            "model": "old",
            "key": "value2"
        });

        let rewritten = rewrite_model_request(body.clone(), &cfg).unwrap();
        assert_eq!(rewritten.model, "young");
        assert_eq!(rewritten.base_url.to_string(), "http://young:8000/");
        assert_eq!(rewritten.body.get("key").unwrap(), "value2");
    }

    #[test]
    fn test_valid_env_var() {
        env::set_var("MODEL_REWRITES", "cat:dog,old:young");
        let result = parse_model_rewrite();

        let mut expected = HashMap::new();
        expected.insert("cat".to_string(), "dog".to_string());
        expected.insert("old".to_string(), "young".to_string());

        assert_eq!(result, expected);
    }

    #[test]
    fn test_empty_env_var() {
        env::set_var("MODEL_REWRITES", "");
        let result = parse_model_rewrite();
        assert!(result.is_empty());
    }

    #[test]
    fn test_invalid_format() {
        env::set_var("MODEL_REWRITES", "cat:dog,invalidpair,old:young");
        let result = parse_model_rewrite();

        let mut expected = HashMap::new();
        expected.insert("cat".to_string(), "dog".to_string());
        expected.insert("old".to_string(), "young".to_string());

        assert_eq!(result, expected);
    }

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
