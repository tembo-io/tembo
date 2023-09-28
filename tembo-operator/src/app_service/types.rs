use k8s_openapi::api::core::v1::ResourceRequirements;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;
// defines a app container
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, JsonSchema)]
pub struct AppService {
    pub name: String,
    pub image: String,
    pub args: Option<Vec<String>>,
    pub command: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
    pub resources: Option<ResourceRequirements>,
    pub probes: Option<Probes>,
    pub routing: Option<Vec<Routing>>,
}


// if there is a Routing port, then a service is created using that Port
// when ingress_path is present, an ingress is created. Otherwise, no ingress is created
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema, JsonSchema)]
pub struct Routing {
    pub port: u16,
    #[serde(rename = "ingressPath")]
    pub ingress_path: Option<String>,
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
pub struct Routes {
    #[serde(rename = "containerPort")]
    pub container_port: u32,
    pub path: String,
}
