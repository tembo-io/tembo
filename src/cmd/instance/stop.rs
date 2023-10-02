// instance stop command
use crate::cli::config::Config;
use crate::cli::docker::Docker;
use anyhow::anyhow;
use clap::{Arg, ArgAction, ArgMatches, Command};
use simplelog::*;
use std::error::Error;

// example usage: tembo instance stop -n my_app_db
pub fn make_subcommand() -> Command {
    Command::new("stop")
        .about("Command used to stop local instances")
        .arg(
            Arg::new("name")
                .short('n')
                .long("name")
                .action(ArgAction::Set)
                .required(true)
                .help("The name of running instance"),
        )
}

pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    let config = Config::new(args, &Config::full_path(args));
    let name = args
        .get_one::<String>("name")
        .ok_or_else(|| anyhow!("Name is missing."))?;

    if config.instances.is_empty() {
        info!("No instances have been configured");
    } else {
        info!("Finding config for {}", name);

        for instance in &config.instances {
            if instance.name.clone().unwrap().to_lowercase() == name.to_lowercase() {
                Docker::stop(name)?;
            }
        }
    }

    Ok(())
}
