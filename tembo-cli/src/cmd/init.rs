use crate::cli::context::{
    tembo_context_file_path, tembo_credentials_file_path, tembo_home_dir, CONTEXT_DEFAULT_TEXT,
    CREDENTIALS_DEFAULT_TEXT,
};
use crate::cli::file_utils::FileUtils;
use crate::tui::confirmation;
use clap::Args;
use std::env;
use std::path::Path;

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

    let cargo_manifest_dir =
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR env var not set");

    let relative_path = "tembo/tembo-cli/examples/single-instance/tembo.toml";
    let filepath = Path::new(&cargo_manifest_dir).join(relative_path);

    if !filepath.exists() {
        return Err(anyhow::anyhow!(
            "The specified file was not found: {}",
            filepath.display()
        ));
    }

    let destination_path = Path::new(&FileUtils::get_current_working_dir()).join("tembo.toml");
    FileUtils::download_file(&filepath, &destination_path, false)?;

    confirmation("Tembo initialized successfully!");

    Ok(())
}
