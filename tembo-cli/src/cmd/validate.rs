use std::{collections::HashMap, fs, path::Path};
use anyhow::Error;
use anyhow::Ok;
use clap::{ArgMatches, Command, Args};
use log::{error, info};
use crate::cli::context::{tembo_context_file_path, tembo_credentials_file_path};
use crate::cli::file_utils::FileUtils;
use crate::cli::tembo_config::InstanceSettings;

/// Validates the tembo.toml file, context file, etc.
#[derive(Args)]
pub struct ValidateCommand {
}

pub fn execute() -> Result<(), anyhow::Error> {
    let mut has_error = false;

    if !Path::new(&tembo_context_file_path()).exists() {
        error!(
            "No {} file exists. Run tembo init first!",
            tembo_context_file_path()
        );
        has_error = true
    }

    if !Path::new(&tembo_credentials_file_path()).exists() {
        error!(
            "No {} file exists. Run tembo init first!",
            tembo_credentials_file_path()
        );
        has_error = true
    }

    if !Path::new(&"tembo.toml").exists() {
        error!("No Tembo file (tembo.toml) exists in this directory!");
        has_error = true
    } else {
        let mut file_path = FileUtils::get_current_working_dir();
        file_path.push_str("/tembo.toml");

        let contents = fs::read_to_string(file_path.clone())?;
        let _: HashMap<String, InstanceSettings> = toml::from_str(&contents)?;
    }

    if !Path::new(&"migrations").exists() {
        error!("No migrations directory exists. Run tembo init first!");
        has_error = true
    }

    if has_error {
        return Err(Error::msg("Fix errors above!"));
    }

    info!("tembo validate ran without any errors!");

    Ok(())
}
