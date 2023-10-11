use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::ResourceRequirements;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const COMPONENT_NAME: &str = "appService";

// defines a app container
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema)]
pub struct AppService {
    pub name: String,
    pub image: String,
    pub args: Option<Vec<String>>,
    pub command: Option<Vec<String>>,
    pub env: Option<Vec<EnvVar>>,
    pub resources: Option<ResourceRequirements>,
    pub probes: Option<Probes>,
    pub middlewares: Option<Vec<Middleware>>,
    pub routing: Option<Vec<Routing>>,
}

// Secrets are injected into the container as environment variables
// ths allows users to map these secrets to environment variable of their choice
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema)]
pub struct EnvVar {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(rename = "valueFromPlatform", skip_serializing_if = "Option::is_none")]
    pub value_from_platform: Option<EnvVarRef>,
}

// we will map these from secrets to env vars, if desired
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema)]
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
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema, PartialEq)]
pub struct HeaderConfig {
    pub name: String,
    #[schemars(schema_with = "preserve_arbitrary")]
    pub config: BTreeMap<String, String>,
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
        let middleware = serde_json::json!({
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
        });

        let mw = serde_json::from_value::<Middleware>(middleware).unwrap();
        match mw {
            Middleware::CustomRequestHeaders(mw) => {
                assert_eq!(mw.name, "my-custom-headers");
                assert_eq!(mw.config["My-New-Header"], "yolo");
                assert_eq!(mw.config["Authorization"], "");
            }
        }
    }
}
