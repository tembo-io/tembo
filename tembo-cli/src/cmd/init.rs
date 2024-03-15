use crate::cli::context::{
    tembo_context_file_path, tembo_credentials_file_path, tembo_home_dir, CONTEXT_DEFAULT_TEXT,
    CREDENTIALS_DEFAULT_TEXT,
};
use crate::cli::file_utils::FileUtils;
use crate::tui::confirmation;
use clap::Args;

pub const TEMBO_DEFAULT_TEXT: &str = r#"[test-instance]
environment = "dev"
instance_name = "test-instance"
cpu = "1"
memory = "2Gi"
storage = "10Gi"
replicas = 1
stack_type = "Standard"

[test-instance.postgres_configurations]
shared_preload_libraries = 'pg_stat_statements'
statement_timeout = 60
pg_partman_bgw.dbname = 'postgres'
pg_partman_bgw.interval = "60"
pg_partman_bgw.role = 'postgres'

[test-instance.extensions.pg_jsonschema]
enabled = true
trunk_project = "pg_jsonschema"
trunk_project_version = "0.1.4"

[test-instance.extensions.pgmq]
enabled = true
trunk_project = "pgmq"
trunk_project_version = "0.33.3"

[test-instance.extensions.pg_partman]
enabled = true
trunk_project = "pg_partman"
trunk_project_version = "4.7.4"
"#;

/// Initializes a local environment. Creates a sample context and configuration files.
#[derive(Args)]
pub struct InitCommand {}

pub fn execute() -> Result<(), anyhow::Error> {
    match FileUtils::create_dir("home directory".to_string(), tembo_home_dir()) {
        Ok(t) => t,
        Err(e) => {
            return Err(e);
        }
    }

    match FileUtils::create_file(
        "context".to_string(),
        tembo_context_file_path(),
        CONTEXT_DEFAULT_TEXT.to_string(),
        false,
    ) {
        Ok(t) => t,
        Err(e) => {
            return Err(e);
        }
    }

    match FileUtils::create_file(
        "credentials".to_string(),
        tembo_credentials_file_path(),
        CREDENTIALS_DEFAULT_TEXT.to_string(),
        false,
    ) {
        Ok(t) => t,
        Err(e) => {
            return Err(e);
        }
    }

    let _ = FileUtils::save_tembo_toml(TEMBO_DEFAULT_TEXT);

    confirmation("Tembo initialized successfully!");

    Ok(())
}
