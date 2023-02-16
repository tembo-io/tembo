use k8s_openapi::{api::core::v1::ResourceRequirements, apimachinery::pkg::api::resource::Quantity};
use std::collections::BTreeMap;

pub fn default_replicas() -> i32 {
    1
}

pub fn default_resources() -> ResourceRequirements {
    let limits: BTreeMap<String, Quantity> = BTreeMap::from([
        ("cpu".to_owned(), Quantity("2".to_string())),
        ("memory".to_owned(), Quantity("2Gi".to_string())),
    ]);
    let requests: BTreeMap<String, Quantity> = BTreeMap::from([
        ("cpu".to_owned(), Quantity("1".to_string())),
        ("memory".to_owned(), Quantity("1Gi".to_string())),
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
    "quay.io/coredb/postgres:c03124e".to_owned()
}

pub fn default_postgres_exporter_image() -> String {
    "quay.io/prometheuscommunity/postgres-exporter:v0.11.1".to_owned()
}

pub fn default_extensions() -> Vec<String> {
    vec![]
}
