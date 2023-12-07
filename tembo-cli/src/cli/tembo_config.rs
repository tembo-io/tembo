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
    pub cpu: String,
    pub memory: String,
    pub storage: String,
    pub replicas: i32,
    pub postgres_configurations: Option<HashMap<String, Value>>,
    pub extensions: Option<HashMap<String, Extension>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Extension {
    pub enabled: bool,
    pub trunk_project: Option<String>,
    pub trunk_project_version: Option<String>,
}
