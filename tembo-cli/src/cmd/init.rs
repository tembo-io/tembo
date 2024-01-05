use crate::cli::context::{
    tembo_context_file_path, tembo_credentials_file_path, tembo_home_dir, CONTEXT_DEFAULT_TEXT,
    CREDENTIALS_DEFAULT_TEXT,
};
use crate::cli::file_utils::FileUtils;
use crate::tui::confirmation;
use clap::Args;
use colorful::Colorful;

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

    let filename = "tembo.toml";
    let filepath =
        "https://raw.githubusercontent.com/tembo-io/tembo/main/tembo-cli/examples/single-instance/tembo.toml";

    FileUtils::download_file(filepath, filename, false)?;

    match FileUtils::create_dir("migrations directory".to_string(), "migrations".to_string()) {
        Ok(t) => t,
        Err(e) => {
            return Err(e);
        }
    }

    confirmation("Tembo initialized successfully!");

    Ok(())
}
