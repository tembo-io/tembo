// extension install command
use crate::cli::config::Config;
use crate::cli::docker::DockerError;
use crate::cli::instance::Instance;
use clap::{Arg, ArgAction, ArgMatches, Command};
use std::error::Error;

// example usage: tembo extension install -n pgmq -i test
pub fn make_subcommand() -> Command {
    Command::new("install")
        .about("Command used to install extensions for instances")
        .arg(
            Arg::new("name")
                .short('n')
                .long("name")
                .action(ArgAction::Set)
                .required(true)
                .help("The name of the extension to install"),
        )
        .arg(
            Arg::new("instance")
                .short('i')
                .long("instance")
                .action(ArgAction::Set)
                .required(true)
                .help("The name of the instance to install the extension on"),
        )
}

pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    // TODO: switch this guard to use anyhow
    if cfg!(target_os = "windows") {
        warn!("{}", crate::WINDOWS_ERROR_MSG);

        return Err(Box::new(DockerError::new(crate::WINDOWS_ERROR_MSG)));
    }

    let config = Config::new(args, &Config::full_path(args));
    let name = args.try_get_one::<String>("name").unwrap();
    let instance = args.try_get_one::<String>("instance").unwrap();

    // TODO: make sure Docker is running

    if config.instances.is_empty() {
        info!("No instances have been configured");
    } else {
        for instance in &config.instances {
            if instance.name.clone().unwrap().to_lowercase() == instance.unwrap().to_lowercase() {
                // TODO: make sure the instance is running

                // TODO: install the extension when the instance is running

                // TODO: ask user if they want it enabled?
            }
        }
    }

    Ok(())
}
