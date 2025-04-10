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

impl Config {
    // Returns true if the configuration uses the Tembo Postgres image.
    pub fn uses_postgres_image(&self) -> bool {
        self.container_image.contains("postgres:")
    }

    // Returns "Always" when the configuration uses the Tembo Postgres image
    // with a specific Postgres major version and OS name, e.g.,
    // "quay.io/tembo/postgres:17-noble". Otherwise it returns
    // "IfNotPresent". The idea is to always pull images for a major Postgres
    // version and OS version to keep Pods up-to-date, but we don't want to
    // pull an image unnecessarily if it doesn't contain the Postgres and OS
    // versions. Because we don't want to change major or OS versions, configs
    // should always use the `postgres:$pg_major-$os_version` tag.
    pub fn image_pull_policy(&self) -> Option<String> {
        let re = regex::Regex::new(r"(?:^|/)postgres:\d+-[a-z]+$").unwrap();
        Some(
            if re.is_match(&self.container_image) {
                "Always"
            } else {
                "IfNotPresent"
            }
            .to_string(),
        )
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_image() {
        for (name, image, uses, always) in [
            ("empty", "", false, false),
            (
                "old_default",
                "quay.io/tembo/tembo-pg-cnpg:15.3.0-5-cede445",
                false,
                false,
            ),
            (
                "standard_sixteen",
                "quay.io/tembo/standard-cnpg:16-ee80907",
                false,
                false,
            ),
            (
                "gis_fourteen",
                "quay.io/tembo/geo-cnpg:14-ee80907",
                false,
                false,
            ),
            (
                "aws_sixteen",
                "387894460527.dkr.ecr.us-east-1.amazonaws.com/tembo-io/standard-cnpg:16.1-d15f2dc",
                false,
                false,
            ),
            (
                "postgres_seventeen_noble",
                "quay.io/tembo/postgres:17-noble",
                true,
                true,
            ),
            (
                "postgres_seventeen_four_noble",
                "quay.io/tembo/postgres:17.4-noble",
                true,
                false,
            ),
            (
                "postgres_fourteen_jammy",
                "quay.io/tembo/postgres:14-jammy",
                true,
                true,
            ),
            (
                "postgres_fourteen_two_jammy",
                "quay.io/tembo/postgres:14.2-jammy",
                true,
                false,
            ),
            ("postgres_sixteen", "quay.io/tembo/postgres:16", true, false),
            (
                "postgres_fifteen_timestamp",
                "quay.io/tembo/postgres:15.12-noble-202503122254",
                true,
                false,
            ),
            (
                "old_default_no_registry",
                "tembo-pg-cnpg:15.3.0-5-cede445",
                false,
                false,
            ),
            (
                "postgres_no_registry",
                "postgres:15.12-noble-202503122254",
                true,
                false,
            ),
        ] {
            let config = Config {
                pod_annotation: "".to_string(),
                namespace_label: "".to_string(),
                server_host: "".to_string(),
                server_port: 5432,
                container_image: image.to_string(),
                init_container_name: "".to_string(),
                tls_cert: "".to_string(),
                tls_key: "".to_string(),
                opentelemetry_endpoint_url: None,
            };
            assert_eq!(uses, config.uses_postgres_image(), "{name}");
            assert_eq!(
                Some(if always { "Always" } else { "IfNotPresent" }.to_string()),
                config.image_pull_policy(),
            );
        }
    }
}
