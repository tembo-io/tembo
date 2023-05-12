use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub enable_initial_backup: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable_initial_backup: from_env_default("ENABLE_INITIAL_BACKUP", "true").parse().unwrap(),
        }
    }
}

// Source the variable from the env - use default if not set
fn from_env_default(var: &str, default: &str) -> String {
    env::var(var).unwrap_or_else(|_| default.to_owned())
}
