use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
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
    pub extra_domains_rw: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct OverlayInstanceSettings {
    pub environment: Option<String>,
    pub instance_name: Option<String>,
    pub cpu: Option<String>,
    pub memory: Option<String>,
    pub storage: Option<String>,
    pub replicas: Option<i32>,
    pub stack_type: Option<String>,
    pub postgres_configurations: Option<HashMap<String, Value>>,
    pub extensions: Option<HashMap<String, Extension>>,
    pub extra_domains_rw: Option<Vec<String>>,
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
    "1GiB".to_string()
}

fn default_storage() -> String {
    "10GiB".to_string()
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
