use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub pod_annotation: String,
    pub namespace_label: String,
    pub server_host: String,
    pub server_port: u16,
    pub container_image: String,
    pub init_container_name: String,
    pub tls_cert: String,
    pub tls_key: String,
    pub opentelemetry_endpoint_url: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pod_annotation: from_env_or_default("POD_ANNOTATION", "tembo-pod-init.tembo.io/inject"),
            namespace_label: from_env_or_default(
                "NAMESPACE_LABEL",
                "tembo-pod-init.tembo.io/watch",
            ),
            server_host: from_env_or_default("SERVER_HOST", "0.0.0.0")
                .parse()
                .unwrap(),
            server_port: from_env_or_default("SERVER_PORT", "8443").parse().unwrap(),
            container_image: from_env_or_default(
                "CONTAINER_IMAGE",
                "quay.io/tembo/tembo-pg-cnpg:latest",
            )
            .parse()
            .unwrap(),
            init_container_name: from_env_or_default("INIT_CONTAINER_NAME", "tembo-bootstrap")
                .parse()
                .unwrap(),
            tls_cert: from_env_or_default("TLS_CERT", "/certs/tls.crt")
                .parse()
                .unwrap(),
            tls_key: from_env_or_default("TLS_KEY", "/certs/tls.key")
                .parse()
                .unwrap(),
            opentelemetry_endpoint_url: {
                let url = std::env::var("OPENTELEMETRY_ENDPOINT_URL").unwrap_or_default();
                if url.is_empty() {
                    None
                } else {
                    Some(url)
                }
            },
        }
    }
}

// Source the variable from the env - use default if not set
fn from_env_or_default(var: &str, default: &str) -> String {
    let value = env::var(var).unwrap_or_else(|_| default.to_owned());
    if value.is_empty() {
        panic!("{} must be set", var);
    }
    value
}
