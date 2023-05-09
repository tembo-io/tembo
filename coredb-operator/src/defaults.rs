use k8s_openapi::{api::core::v1::ResourceRequirements, apimachinery::pkg::api::resource::Quantity};
use std::collections::BTreeMap;

use crate::{
    apis::coredb_types::{Backup, ServiceAccountTemplate},
    extensions::Extension,
};

pub fn default_replicas() -> i32 {
    1
}

pub fn default_resources() -> ResourceRequirements {
    let limits: BTreeMap<String, Quantity> = BTreeMap::from([
        ("cpu".to_owned(), Quantity("2".to_string())),
        ("memory".to_owned(), Quantity("2Gi".to_string())),
    ]);
    let requests: BTreeMap<String, Quantity> = BTreeMap::from([
        ("cpu".to_owned(), Quantity("500m".to_string())),
        ("memory".to_owned(), Quantity("512Mi".to_string())),
    ]);
    ResourceRequirements {
        limits: Some(limits),
        requests: Some(requests),
    }
}

pub fn default_postgres_exporter_enabled() -> bool {
    true
}

pub fn default_uid() -> i32 {
    999
}

pub fn default_port() -> i32 {
    5432
}

pub fn default_image() -> String {
    "quay.io/coredb/coredb-pg-slim:0a1db4d".to_owned()
}

pub fn default_storage() -> Quantity {
    Quantity("8Gi".to_string())
}

pub fn default_sharedir_storage() -> Quantity {
    Quantity("1Gi".to_string())
}

pub fn default_pkglibdir_storage() -> Quantity {
    Quantity("1Gi".to_string())
}

pub fn default_postgres_exporter_image() -> String {
    "quay.io/prometheuscommunity/postgres-exporter:v0.11.1".to_owned()
}

pub fn default_extensions() -> Vec<Extension> {
    vec![]
}

pub fn default_database() -> String {
    "postrgres".to_owned()
}

pub fn default_schema() -> String {
    "public".to_owned()
}

pub fn default_description() -> String {
    "No description provided".to_owned()
}

pub fn default_stop() -> bool {
    false
}

pub fn default_extensions_updating() -> bool {
    false
}

pub fn default_service_account_template() -> ServiceAccountTemplate {
    ServiceAccountTemplate { metadata: None }
}

pub fn default_backup() -> Option<Backup> {
    None
}

pub fn default_encryption() -> Option<String> {
    Some("AES256".to_owned())
}

pub fn default_retention_policy() -> Option<String> {
    Some("30d".to_owned())
}

pub fn default_backup_schedule() -> Option<String> {
    // Every day at midnight
    Some("0 0 * * *".to_owned())
}
