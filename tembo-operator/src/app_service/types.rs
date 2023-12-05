use std::collections::BTreeMap;

use k8s_openapi::{
    api::core::v1::{ResourceRequirements, Volume, VolumeMount},
    apimachinery::pkg::api::resource::Quantity,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const COMPONENT_NAME: &str = "appService";

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct StorageConfig {
    pub volumes: Option<Vec<Volume>>,
    #[serde(rename = "volumeMounts")]
    pub volume_mounts: Option<Vec<VolumeMount>>,
}

// defines a app container
#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct AppService {
    pub name: String,
    pub image: String,
    pub args: Option<Vec<String>>,
    pub command: Option<Vec<String>>,
    pub env: Option<Vec<EnvVar>>,
    #[serde(default = "default_resources")]
    pub resources: ResourceRequirements,
    pub probes: Option<Probes>,
    pub middlewares: Option<Vec<Middleware>>,
    pub routing: Option<Vec<Routing>>,
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

// if there is a Routing port, then a service is created using that Port
// when ingress_path is present, an ingress is created. Otherwise, no ingress is created
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema, JsonSchema)]
pub struct Routing {
    pub port: u16,
    #[serde(rename = "ingressPath")]
    pub ingress_path: Option<String>,
    // provide name of the middleware resources to apply to this route
    pub middlewares: Option<Vec<String>>,
    #[serde(rename = "entryPoints")]
    #[serde(default = "default_entry_points")]
    pub entry_points: Option<Vec<String>>,
}

pub fn default_entry_points() -> Option<Vec<String>> {
    Some(vec!["websecure".to_owned()])
}

#[allow(non_snake_case)]
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct Probes {
    pub readiness: Probe,
    pub liveness: Probe,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct Probe {
    pub path: String,
    pub port: String,
    // this should never be negative
    #[serde(rename = "initialDelaySeconds")]
    pub initial_delay_seconds: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct Ingress {
    pub enabled: bool,
    pub path: Option<String>,
}

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
