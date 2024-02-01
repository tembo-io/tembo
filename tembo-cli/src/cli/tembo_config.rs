use k8s_openapi::api::core::v1::{ResourceRequirements, Volume, VolumeMount};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, HashMap};
use toml::Value;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TemboConfig {
    pub version: String,
    pub defaults: InstanceSettings,
}

// Config struct holds to data from the `[config]` section.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct InstanceSettings {
    pub environment: String,
    pub instance_name: String,
    #[serde(default = "default_cpu")]
    pub cpu: String,
    #[serde(default = "default_memory")]
    pub memory: String,
    #[serde(default = "default_storage")]
    pub storage: String,
    #[serde(default = "default_replicas")]
    pub replicas: i32,
    #[serde(default = "default_stack_type")]
    pub stack_type: String,
    pub postgres_configurations: Option<HashMap<String, Value>>,
    #[serde(
        deserialize_with = "deserialize_extensions",
        default = "default_extensions"
    )]
    pub extensions: Option<HashMap<String, Extension>>,
    pub app_services: Option<HashMap<String, AppType>>,
    pub extra_domains_rw: Option<Vec<String>>,
    pub ip_allow_list: Option<Vec<String>>,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AppType {
    #[serde(rename = "restapi")]
    RestAPI(Option<AppConfig>),
    #[serde(rename = "http")]
    HTTP(Option<AppConfig>),
    #[serde(rename = "mq-api")]
    MQ(Option<AppConfig>),
    #[serde(rename = "embeddings")]
    Embeddings(Option<AppConfig>),
    #[serde(rename = "custom")]
    Custom(AppService),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub env: Option<Vec<EnvVar>>,
    pub resources: Option<ResourceRequirements>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct AppService {
    pub name: String,

    pub image: String,

    pub args: Option<Vec<String>>,

    pub command: Option<Vec<String>>,

    pub env: Option<Vec<EnvVar>>,

    pub resources: ResourceRequirements,

    pub probes: Option<Probes>,

    pub middlewares: Option<Vec<Middleware>>,

    pub routing: Option<Vec<Routing>>,

    pub storage: Option<StorageConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EnvVar {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(rename = "valueFromPlatform", skip_serializing_if = "Option::is_none")]
    pub value_from_platform: Option<EnvVarRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum EnvVarRef {
    ReadOnlyConnection,
    ReadWriteConnection,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Probes {
    pub readiness: Probe,
    pub liveness: Probe,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Probe {
    pub path: String,
    pub port: String,
    // this should never be negative
    #[serde(rename = "initialDelaySeconds")]
    pub initial_delay_seconds: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Middleware {
    CustomRequestHeaders(HeaderConfig),
    StripPrefix(StripPrefixConfig),
    ReplacePathRegex(ReplacePathRegexConfig),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HeaderConfig {
    pub name: String,
    pub config: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StripPrefixConfig {
    pub name: String,
    pub config: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReplacePathRegexConfig {
    pub name: String,
    pub config: ReplacePathRegexConfigType,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReplacePathRegexConfigType {
    pub regex: String,
    pub replacement: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Routing {
    pub port: u16,
    pub ingress_path: Option<String>,

    /// provide name of the middleware resources to apply to this route
    pub middlewares: Option<Vec<String>>,
    pub entry_points: Option<Vec<String>>,
    pub ingress_type: Option<IngressType>,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum IngressType {
    http,
    tcp,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StorageConfig {
    pub volumes: Option<Vec<Volume>>,
    #[serde(rename = "volumeMounts")]
    pub volume_mounts: Option<Vec<VolumeMount>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct OverlayInstanceSettings {
    pub cpu: Option<String>,
    pub memory: Option<String>,
    pub storage: Option<String>,
    pub replicas: Option<i32>,
    pub stack_type: Option<String>,
    pub postgres_configurations: Option<HashMap<String, Value>>,
    pub extensions: Option<HashMap<String, Extension>>,
    pub extra_domains_rw: Option<Vec<String>>,
    pub ip_allow_list: Option<Vec<String>>,
}

// If a trunk project name is not specified, then assume
// it's the same name as the extension.
fn deserialize_extensions<'de, D>(
    deserializer: D,
) -> Result<Option<HashMap<String, Extension>>, D::Error>
where
    D: Deserializer<'de>,
{
    let map = Option::<HashMap<String, Extension>>::deserialize(deserializer)?;

    map.map(|mut m| {
        m.iter_mut().for_each(|(key, ext)| {
            if ext.trunk_project.is_none() {
                ext.trunk_project = Some(key.clone());
            }
        });
        m
    })
    .map_or(Ok(None), |m| Ok(Some(m)))
}

fn default_cpu() -> String {
    "0.25".to_string()
}

fn default_memory() -> String {
    "1Gi".to_string()
}

fn default_storage() -> String {
    "10Gi".to_string()
}

fn default_replicas() -> i32 {
    1
}

fn default_stack_type() -> String {
    "Standard".to_string()
}

fn default_extensions() -> Option<HashMap<String, Extension>> {
    Some(HashMap::new())
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Extension {
    pub enabled: bool,
    pub trunk_project: Option<String>,
    pub trunk_project_version: Option<String>,
}
