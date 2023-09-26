// instance start command
use crate::cli::config::Config;
use crate::cli::docker::Docker;
use crate::cli::docker::DockerError;
use anyhow::anyhow;
use clap::{Arg, ArgAction, ArgMatches, Command};
use simplelog::*;
use spinners::{Spinner, Spinners};
use std::error::Error;

// example usage: tembo instance start -n my_app_db
pub fn make_subcommand() -> Command {
    Command::new("start")
        .about("Command used to start local instances")
        .arg(
            Arg::new("name")
                .short('n')
                .long("name")
                .action(ArgAction::Set)
                .required(true)
                .help("The name you want to use for this instance"),
        )
}

pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    if cfg!(target_os = "windows") {
        warn!("{}", crate::WINDOWS_ERROR_MSG);

        return Err(Box::new(DockerError::new(crate::WINDOWS_ERROR_MSG)));
    }

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
                info!(" config has been found");
                info!(" starting via Docker");

                Docker::start(name, instance)?;
            }
        }
    }

    Ok(())
}
