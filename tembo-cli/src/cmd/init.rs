use crate::cli::context::{
    tembo_context_file_path, tembo_credentials_file_path, tembo_home_dir, CONTEXT_DEFAULT_TEXT,
    CONTEXT_EXAMPLE_TEXT, CREDENTIALS_DEFAULT_TEXT, CREDENTIALS_EXAMPLE_TEXT,
};
use crate::cli::file_utils::FileUtils;
use crate::tui::confirmation;
use clap::Args;
use std::env;

/// Initializes a local environment. Creates a sample context and configuration files.
#[derive(Args)]
pub struct InitCommand {}

pub fn execute() -> Result<(), anyhow::Error> {
    // Determine if running tests
    let is_test_env = cfg!(test) || env::var("RUNNING_TESTS").is_ok();

    let context_text = if is_test_env {
        CONTEXT_EXAMPLE_TEXT
    } else {
        CONTEXT_DEFAULT_TEXT
    };

    let credentials_text = if is_test_env {
        CREDENTIALS_EXAMPLE_TEXT
    } else {
        CREDENTIALS_DEFAULT_TEXT
    };

    print!("{}", context_text);

    match FileUtils::create_dir("home directory".to_string(), tembo_home_dir()) {
        Ok(t) => t,
        Err(e) => {
            return Err(e);
        }
    }

    match FileUtils::create_file(
        "context".to_string(),
        tembo_context_file_path(),
        CONTEXT_EXAMPLE_TEXT.to_string(),
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
        CREDENTIALS_EXAMPLE_TEXT.to_string(),
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

    confirmation("Tembo initialized successfully!");

    Ok(())
}
