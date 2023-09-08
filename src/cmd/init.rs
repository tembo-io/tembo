use crate::cli::config::Config;
use crate::cli::docker::Docker;
use clap::{ArgMatches, Command};
use std::error::Error;

// Create clap subcommand arguments
pub fn make_subcommand() -> Command {
    Command::new("init")
        .about("Initializes a local environment or project; generates configuration")
}

pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    let config = Config::new(args, &Config::full_path(args));

    println!(
        "- Config file created at: {}",
        &config.created_at.to_string()
    );

    match check_requirements() {
        Ok(_) => println!("- Docker was found and appears running"),
        Err(e) => {
            return Err(e);
        }
    }

    Ok(())
}

fn check_requirements() -> Result<(), Box<dyn Error>> {
    Docker::installed_and_running()
}
