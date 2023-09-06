use chrono::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use std::cmp::PartialEq;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Cluster {
    pub name: Option<String>,
    pub r#type: Option<String>,
    pub version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub installed_extensions: Vec<InstalledExtensions>,
    pub enabled_extensions: Vec<EnabledExtensions>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct InstalledExtensions {
    pub name: Option<String>,
    pub version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct EnabledExtensions {
    pub name: Option<String>,
    pub version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}
