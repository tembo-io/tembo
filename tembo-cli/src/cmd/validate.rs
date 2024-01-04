use crate::cli::context::{tembo_context_file_path, tembo_credentials_file_path};
use crate::cli::file_utils::FileUtils;
use crate::cli::tembo_config::InstanceSettings;
use crate::tui::white_confirmation;
use anyhow::Error;
use anyhow::Ok;
use clap::Args;
use std::{collections::HashMap, fs, path::Path};

/// Validates the tembo.toml file, context file, etc.
#[derive(Args)]
pub struct ValidateCommand {}

pub fn execute(verbose: bool) -> Result<(), anyhow::Error> {
    let mut has_error = false;

    if !Path::new(&tembo_context_file_path()).exists() {
        println!(
            "No {} file exists. Run tembo init first!",
            tembo_context_file_path()
        );
        has_error = true
    }
    if verbose {
        println!("- Context file exists");
    }

    if !Path::new(&tembo_credentials_file_path()).exists() {
        println!(
            "No {} file exists. Run tembo init first!",
            tembo_credentials_file_path()
        );
        has_error = true
    }
    if verbose {
        println!("- Credentials file exists");
    }

    if !Path::new(&"tembo.toml").exists() {
        println!("No Tembo file (tembo.toml) exists in this directory!");
        has_error = true
    } else {
        let mut file_path = FileUtils::get_current_working_dir();
        file_path.push_str("/tembo.toml");

        let contents = fs::read_to_string(file_path.clone())?;
        let _: HashMap<String, InstanceSettings> = toml::from_str(&contents)?;
    }
    if verbose {
        println!("- Tembo file exists");
    }

    if has_error {
        return Err(Error::msg("Fix errors above!"));
    }

    white_confirmation("Configuration is valid");

    Ok(())
}
