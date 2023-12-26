use serde::{Deserialize, Serialize};
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
    pub extensions: Option<HashMap<String, Extension>>,
    pub extra_domains_rw: Option<Vec<String>>,
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Extension {
    pub enabled: bool,
    pub trunk_project: Option<String>,
    pub trunk_project_version: Option<String>,
}
