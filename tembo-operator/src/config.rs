use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub cloud_provider: String,
    pub enable_backup: bool,
    pub enable_volume_snapshot: bool,
    pub reconcile_timestamp_ttl: u64,
    pub reconcile_ttl: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cloud_provider: from_env_default("CLOUD_PROVIDER", "aws"),
            enable_backup: from_env_default("ENABLE_BACKUP", "true").parse().unwrap(),
            enable_volume_snapshot: from_env_default("ENABLE_VOLUME_SNAPSHOT", "false")
                .parse()
                .unwrap(),
            // The time to live for recociling the reconcile timestamp
            reconcile_timestamp_ttl: from_env_default("RECONCILE_TIMESTAMP_TTL", "30")
                .parse()
                .unwrap(),
            // The time to live for reconciling the entire instance
            reconcile_ttl: from_env_default("RECONCILE_TTL", "90").parse().unwrap(),
        }
    }
}

// Source the variable from the env - use default if not set
fn from_env_default(var: &str, default: &str) -> String {
    env::var(var).unwrap_or_else(|_| default.to_owned())
}
