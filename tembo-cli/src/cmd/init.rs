use crate::cli::context::{
    tembo_context_file_path, tembo_credentials_file_path, tembo_home_dir, CONTEXT_DEFAULT_TEXT,
    CREDENTIALS_DEFAULT_TEXT,
};
use crate::cli::file_utils::FileUtils;
use clap::Args;

pub const TEMBO_DEFAULT_TEXT: &str = r#"[test-instance]
environment = "prod"
instance_name = "test-instance"
cpu = "0.25"
memory = "1Gi"
storage = "10Gi"
replicas = 1
stack_type = "Standard"
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

    Ok(())
}
