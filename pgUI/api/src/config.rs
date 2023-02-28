use std::env;

#[derive(Debug)]
pub struct Config {
    pub pg_conn_str: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pg_conn_str: from_env_default(
                "POSTGRES_CONNECTION",
                "postgresql://postgres:postgres@0.0.0.0:5432/postgres",
            ),
        }
    }
}

/// source a variable from environment - use default if not exists
fn from_env_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}
