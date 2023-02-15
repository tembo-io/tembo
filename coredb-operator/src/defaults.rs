pub fn default_replicas() -> i32 {
    1
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
    "quay.io/coredb/postgres:a703af3".to_owned()
}

pub fn default_postgres_exporter_image() -> String {
    "quay.io/prometheuscommunity/postgres-exporter:v0.11.1".to_owned()
}

pub fn default_extensions() -> Vec<String> {
    vec![]
}
