// Objects representing a user created local instance of a stack
// (a local container that runs with certain attributes and properties)

use chrono::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use std::cmp::PartialEq;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Instance {
    pub name: Option<String>,
    pub r#type: Option<String>,
    pub port: Option<String>, // TODO: persist as an <u16>
    pub version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub installed_extensions: Vec<InstalledExtension>,
    pub enabled_extensions: Vec<EnabledExtension>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledExtension {
    pub name: Option<String>,
    pub version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnabledExtension {
    pub name: Option<String>,
    pub version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub locations: Vec<ExtensionLocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionLocation {
    pub database: String,
    pub enabled: String,
    pub version: String,
}
