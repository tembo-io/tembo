use controller::app_service::types::AppService;
use controller::extensions::types::Extension as ControllerExtension;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use tembo_stacks::apps::types::AppType;
use toml::Value;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TemboConfig {
    pub version: String,
    pub defaults: InstanceSettings,
}

// Config struct holds to data from the `[config]` section.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
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
    pub stack_type: Option<String>,
    pub postgres_configurations: Option<HashMap<String, Value>>,
    #[serde(default = "default_pg_version")]
    pub pg_version: u8,
    #[serde(
        deserialize_with = "deserialize_extensions",
        default = "default_extensions"
    )]
    pub extensions: Option<HashMap<String, Extension>>,
    pub app_services: Option<Vec<AppType>>,
    pub controller_app_services: Option<HashMap<String, AppService>>,
    pub final_extensions: Option<Vec<ControllerExtension>>,
    pub extra_domains_rw: Option<Vec<String>>,
    pub ip_allow_list: Option<Vec<String>>,
    pub stack_file: Option<String>,
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
    pub pg_version: Option<u8>,
    pub stack_file: Option<String>,
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

/// Default to Postgres 15
fn default_pg_version() -> u8 {
    15
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

fn default_extensions() -> Option<HashMap<String, Extension>> {
    Some(HashMap::new())
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Extension {
    pub version: Option<String>,
    pub enabled: bool,
    pub trunk_project: Option<String>,
    pub trunk_project_version: Option<String>,
}

#[derive(Clone)]
pub struct Library {
    pub name: String,
    pub priority: i32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TrunkProject {
    pub name: String,
    pub extensions: Option<Vec<TrunkExtension>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TrunkExtension {
    pub extension_name: String,
    pub loadable_libraries: Option<Vec<LoadableLibrary>>,
    pub version: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LoadableLibrary {
    pub library_name: String,
    pub priority: i32,
}
