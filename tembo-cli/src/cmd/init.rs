use crate::{
    cli::context::{tembo_state_file_path, DOT_TEMBO_FOLDER},
    Result,
};
use clap::{ArgMatches, Command};

use crate::cli::{
    context::{
        tembo_context_file_path, tembo_credentials_file_path, tembo_home_dir, CONTEXT_DEFAULT_TEXT,
        CREDENTIALS_DEFAULT_TEXT,
    },
    file_utils::FileUtils,
};

// Create init subcommand arguments
pub fn make_subcommand() -> Command {
    Command::new("init")
        .about("Initializes a local environment; creates needed context & config files/directories")
}

pub fn execute(_args: &ArgMatches) -> Result<()> {
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

    match FileUtils::create_file(
        "config".to_string(),
        "tembo.toml".to_string(),
        "".to_string(),
        false,
    ) {
        Ok(t) => t,
        Err(e) => {
            return Err(e);
        }
    }

    match FileUtils::create_dir("migrations directory".to_string(), "migrations".to_string()) {
        Ok(t) => t,
        Err(e) => {
            return Err(e);
        }
    }

    match FileUtils::create_dir(".tembo directory".to_string(), DOT_TEMBO_FOLDER.to_string()) {
        Ok(t) => t,
        Err(e) => {
            return Err(e);
        }
    }

    match FileUtils::create_file(
        tembo_state_file_path(),
        tembo_state_file_path(),
        "".to_string(),
        false,
    ) {
        Ok(t) => t,
        Err(e) => {
            return Err(e);
        }
    }

    Ok(())
}
