pub fn default_replicas() -> i32 {
    1
}

pub fn default_uid() -> i32 {
    999
}

pub fn default_port() -> i32 {
    5432
}

pub fn default_image() -> String {
    "quay.io/coredb/postgres:2023.01.24".to_owned()
}

pub fn default_extensions() -> Vec<String> {
    vec![]
}
