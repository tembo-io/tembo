use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub enable_backup: bool,
    pub enable_volume_snapshot: bool,
    pub volume_snapshot_retention_period_days: u64,
    pub reconcile_ttl: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable_backup: from_env_default("ENABLE_BACKUP", "true").parse().unwrap(),
            enable_volume_snapshot: from_env_default("ENABLE_VOLUME_SNAPSHOT", "false")
                .parse()
                .unwrap(),
            volume_snapshot_retention_period_days: from_env_default(
                "VOLUME_SNAPSHOT_RETENTION_PERIOD_DAYS",
                "1",
            )
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
