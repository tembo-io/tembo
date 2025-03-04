use std::collections::BTreeMap;

use k8s_openapi::{
    api::core::v1::{ResourceRequirements, Volume, VolumeMount},
    apimachinery::pkg::api::resource::Quantity,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const COMPONENT_NAME: &str = "appService";

/// StorageConfig is used to configure the storage for the appService.
/// This uses the `Volume` and `VolumeMount` types from the Kubernetes API.
///
/// See the [Kubernetes docs](https://kubernetes.io/docs/concepts/storage/volumes/).
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct StorageConfig {
    pub volumes: Option<Vec<Volume>>,
    #[serde(rename = "volumeMounts")]
    pub volume_mounts: Option<Vec<VolumeMount>>,
}

/// AppService significantly extends the functionality of your Tembo Postgres
/// instance by running tools and software built by the Postgres open source community.
///
/// **Example**: This will configure and install a PostgREST container along side
/// the Postgres instance, install pg_graphql extension, and configure the
/// ingress routing to expose the PostgREST service.
///
/// ```yaml
/// apiVersion: coredb.io/v1alpha1
/// kind: CoreDB
/// metadata:
///   name: test-db
/// spec:
///   trunk_installs:
///     - name: pg_graphql
///       version: 1.2.0
///   extensions:
///     - name: pg_graphql
///       locations:
///       - database: postgres
///         enabled: true
///  
///   appServices:
///     - name: postgrest
///       image: postgrest/postgrest:v12.2.8
///       routing:
///       # only expose /rest/v1 and /graphql/v1
///         - port: 3000
///           ingressPath: /rest/v1
///           middlewares:
///             - my-headers
///         - port: 3000
///           ingressPath: /graphql/v1
///           middlewares:
///             - map-gql
///             - my-headers
///       middlewares:
///         - customRequestHeaders:
///           name: my-headers
///           config:
///             # removes auth header from request
///             Authorization: ""
///             Content-Profile: graphql
///             Accept-Profile: graphql
///         - stripPrefix:
///           name: my-strip-prefix
///           config:
///             - /rest/v1
///         # reroute gql and rest requests
///         - replacePathRegex:
///           name: map-gql
///           config:
///             regex: \/graphql\/v1\/?
///             replacement: /rpc/resolve
///       env:
///         - name: PGRST_DB_URI
///           valueFromPlatform: ReadWriteConnection
///         - name: PGRST_DB_SCHEMA
///           value: "public, graphql"
///         - name: PGRST_DB_ANON_ROLE
///           value: postgres
///         - name: PGRST_LOG_LEVEL
///           value: info
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct AppService {
    /// Defines the name of the appService.
    pub name: String,

    /// Defines the container image to use for the appService.
    pub image: String,

    /// Defines the arguments to pass into the container if needed.
    /// You define this in the same manner as you would for all Kubernetes containers.
    /// See the [Kubernetes docs](https://kubernetes.io/docs/tasks/inject-data-application/define-command-argument-container).
    pub args: Option<Vec<String>>,

    /// Defines the command into the container if needed.
    /// You define this in the same manner as you would for all Kubernetes containers.
    /// See the [Kubernetes docs](https://kubernetes.io/docs/tasks/inject-data-application/define-command-argument-container).
    pub command: Option<Vec<String>>,

    /// Defines the environment variables to pass into the container if needed.
    /// You define this in the same manner as you would for all Kubernetes containers.
    /// See the [Kubernetes docs](https://kubernetes.io/docs/tasks/inject-data-application/define-environment-variable-container).
    pub env: Option<Vec<EnvVar>>,

    /// Defines the resources to allocate to the container.
    /// You define this in the same manner as you would for all Kubernetes containers.
    /// See the [Kubernetes docs](https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/).
    #[serde(default = "default_resources")]
    pub resources: ResourceRequirements,

    /// Defines the probes to use for the container.
    /// You define this in the same manner as you would for all Kubernetes containers.
    /// See the [Kubernetes docs](https://kubernetes.io/docs/tasks/configure-pod-container/configure-liveness-readiness-startup-probes/).
    pub probes: Option<Probes>,

    /// Defines the metrics endpoints to be scraped by Prometheus.
    /// This implements a subset of features available by PodMonitorPodMetricsEndpoints.
    pub metrics: Option<AppMetrics>,

    /// Defines the ingress middeware configuration for the appService.
    /// This is specifically configured for the ingress controller Traefik.
    pub middlewares: Option<Vec<Middleware>>,

    /// Defines the routing configuration for the appService.
    pub routing: Option<Vec<Routing>>,

    /// Defines the storage configuration for the appService.
    pub storage: Option<StorageConfig>,
}

pub fn default_resources() -> ResourceRequirements {
    let limits: BTreeMap<String, Quantity> = BTreeMap::from([
        ("cpu".to_owned(), Quantity("400m".to_string())),
        ("memory".to_owned(), Quantity("256Mi".to_string())),
    ]);
    let requests: BTreeMap<String, Quantity> = BTreeMap::from([
        ("cpu".to_owned(), Quantity("100m".to_string())),
        ("memory".to_owned(), Quantity("256Mi".to_string())),
    ]);
    ResourceRequirements {
        limits: Some(limits),
        requests: Some(requests),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct AppMetrics {
    /// port must be also exposed in one of AppService.routing[]
    pub port: u16,
    /// path to scrape metrics
    pub path: String,
}

// Secrets are injected into the container as environment variables
// ths allows users to map these secrets to environment variable of their choice
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct EnvVar {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(rename = "valueFromPlatform", skip_serializing_if = "Option::is_none")]
    pub value_from_platform: Option<EnvVarRef>,
}

// we will map these from secrets to env vars, if desired
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub enum EnvVarRef {
    ReadOnlyConnection,
    ReadWriteConnection,
}

/// Routing is used if there is a routing port, then a service is created using
/// that Port when ingress_path is present, an ingress is created. Otherwise, no
/// ingress is created
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema, JsonSchema)]
pub struct Routing {
    pub port: u16,
    #[serde(rename = "ingressPath")]
    pub ingress_path: Option<String>,

    /// provide name of the middleware resources to apply to this route
    pub middlewares: Option<Vec<String>>,
    #[serde(rename = "entryPoints")]
    #[serde(default = "default_entry_points")]
    pub entry_points: Option<Vec<String>>,
    #[serde(rename = "ingressType")]
    #[serde(default = "default_ingress_type")]
    pub ingress_type: Option<IngressType>,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema, JsonSchema)]
pub enum IngressType {
    http,
    tcp,
}

pub fn default_ingress_type() -> Option<IngressType> {
    Some(IngressType::http)
}

pub fn default_entry_points() -> Option<Vec<String>> {
    Some(vec!["websecure".to_owned()])
}

/// Probes are used to determine the health of a container.
/// You define this in the same manner as you would for all Kubernetes containers.
/// See the [Kubernetes docs](https://kubernetes.io/docs/tasks/configure-pod-container/configure-liveness-readiness-startup-probes/).
#[allow(non_snake_case)]
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct Probes {
    pub readiness: Probe,
    pub liveness: Probe,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct Probe {
    pub path: String,
    pub port: i32,
    // this should never be negative
    #[serde(rename = "initialDelaySeconds")]
    pub initial_delay_seconds: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct Ingress {
    pub enabled: bool,
    pub path: Option<String>,
}

/// Midddleware is used to configure the middleware for the appService.
/// This is specifically configured for the ingress controller Traefik.
///
/// Please refer to the example in the `AppService` documentation.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub enum Middleware {
    #[serde(rename = "customRequestHeaders")]
    CustomRequestHeaders(HeaderConfig),
    #[serde(rename = "stripPrefix")]
    StripPrefix(StripPrefixConfig),
    #[serde(rename = "replacePathRegex")]
    ReplacePathRegex(ReplacePathRegexConfig),
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct HeaderConfig {
    pub name: String,
    #[schemars(schema_with = "preserve_arbitrary")]
    pub config: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct StripPrefixConfig {
    pub name: String,
    pub config: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct ReplacePathRegexConfig {
    pub name: String,
    pub config: ReplacePathRegexConfigType,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct ReplacePathRegexConfigType {
    pub regex: String,
    pub replacement: String,
}

// source: https://github.com/kube-rs/kube/issues/844
fn preserve_arbitrary(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
    let mut obj = schemars::schema::SchemaObject::default();
    obj.extensions
        .insert("x-kubernetes-preserve-unknown-fields".into(), true.into());
    schemars::schema::Schema::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_middleware_config() {
        let middleware = serde_json::json!([
        {
            "customRequestHeaders": {
                "name": "my-custom-headers",
                "config":
                    {
                        //remove a header
                        "Authorization": "",
                        // add a header
                        "My-New-Header": "yolo"
                    }
            },
        },
        {
            "stripPrefix": {
                "name": "strip-my-prefix",
                "config": [
                    "/removeMe"
                ]
            },
        },
        {
            "replacePathRegex": {
                "name": "replace-my-regex",
                "config":
                    {
                        "regex": "/replace/me",
                        "replacement": "/with/me"
                    }
            },
        }
        ]);

        let mws = serde_json::from_value::<Vec<Middleware>>(middleware).unwrap();
        for mw in mws {
            match mw {
                Middleware::CustomRequestHeaders(mw) => {
                    assert_eq!(mw.name, "my-custom-headers");
                    assert_eq!(mw.config["My-New-Header"], "yolo");
                    assert_eq!(mw.config["Authorization"], "");
                }
                Middleware::StripPrefix(mw) => {
                    assert_eq!(mw.name, "strip-my-prefix");
                    assert_eq!(mw.config[0], "/removeMe");
                }
                Middleware::ReplacePathRegex(mw) => {
                    assert_eq!(mw.name, "replace-my-regex");
                    assert_eq!(mw.config.regex, "/replace/me");
                    assert_eq!(mw.config.replacement, "/with/me");
                }
            }
        }

        // malformed middlewares
        let unsupported_mw = serde_json::json!({
            "unsupportedMiddlewareType": {
                "name": "my-custom-headers",
                "config":
                    {
                        //remove a header
                        "Authorization": "",
                        // add a header
                        "My-New-Header": "yolo"
                    }
            },
        });
        let failed = serde_json::from_value::<Middleware>(unsupported_mw);
        assert!(failed.is_err());

        // provide a supported middleware but with malformed configuration
        let supported_bad_config = serde_json::json!({
            "replacePath": {
                "name": "my-custom-headers",
                "config":
                    {
                        "replacePath": "expects_a_vec<string>",
                    }
            },
        });
        let failed = serde_json::from_value::<Middleware>(supported_bad_config);
        assert!(failed.is_err());
    }
}
