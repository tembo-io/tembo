use crate::cli::config::Config;
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
        "- config file created at: {}",
        &config.created_at.to_string()
    );

    Ok(())
}
