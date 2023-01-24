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
    "docker.io/postgres:15".to_owned()
}

pub fn default_extensions() -> Vec<String> {vec![]}
