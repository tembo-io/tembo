//!  extension install command

use crate::cli::config::Config;
use crate::cli::instance::{InstalledExtension, Instance};
use crate::cli::stacks::TrunkInstall;
use crate::Result;
use anyhow::bail;
use chrono::Utc;
use clap::{Arg, ArgAction, ArgMatches, Command};
use simplelog::*;
use std::io;

// example usage: tembo extension install -n pgmq -i my_instance
pub fn make_subcommand() -> Command {
    Command::new("install")
        .about("Command used to install extensions for instances")
        .arg(
            Arg::new("instance")
                .short('i')
                .long("instance")
                .action(ArgAction::Set)
                .required(true)
                .help("The name of the instance to install the extension for"),
        )
}

pub fn execute(args: &ArgMatches) -> Result<()> {
    let config = Config::new(args, &Config::full_path(args));
    let instance_arg = args.try_get_one::<String>("instance").unwrap();

    println!("What extension would you like to install? Example: pgmq");
    let mut name_str = String::from("");

    io::stdin()
        .read_line(&mut name_str)
        .expect("Failed to read line");
    let name_str = name_str.trim().to_string().replace('\n', "");

    println!(
        "trying to install extension '{}' on instance '{}'",
        &name_str,
        &instance_arg.unwrap()
    );

    if config.instances.is_empty() {
        println!("- No instances have been configured");
    } else {
        let instance = Instance::find(args, instance_arg.unwrap())?;

        install_extension(instance, &name_str, args)?;
    }

    Ok(())
}

fn install_extension(instance: Instance, name: &str, args: &ArgMatches) -> Result<()> {
    println!("What version would you like to install? Example: 2.1.0");
    let mut version_str = String::from("");

    io::stdin()
        .read_line(&mut version_str)
        .expect("Failed to read line");
    let version_str = version_str.trim().to_string().replace('\n', "");

    // TODO: decide if this should just prompt the user to start the instance first
    instance.start();

    for extension in &instance.installed_extensions {
        // TODO: make sure the version is the same, what to do if it is not?
        if extension.name.clone().unwrap().to_lowercase() == name.to_lowercase() {
            warn!(
                "extension {} is already installed for instance {}, remove first before upgrading version",
                &name,
                &instance.name.clone().unwrap()
            );
        } else {
            // try installing extension unless already installed
            let trunk_install = TrunkInstall {
                name: Some(name.to_string()),
                version: Some(version_str.clone().to_string()),
                created_at: Some(Utc::now()),
            };

            match instance.install_extension(&trunk_install) {
                Ok(()) => {
                    info!("extension {} installed", name);
                    let _ = persist_config(args, trunk_install);

                    // TODO: provide feedback on enabling the extension once enable action is in place
                    break;
                }
                Err(e) => error!("there was an error: {}", e),
            }
        }
    }

    Ok(())
}

fn persist_config(args: &ArgMatches, trunk_install: TrunkInstall) -> Result<()> {
    let mut config = Config::new(args, &Config::full_path(args));
    let target_instance = args.try_get_one::<String>("instance");
    let installed_extension = InstalledExtension {
        name: trunk_install.name,
        version: trunk_install.version,
        created_at: trunk_install.created_at,
    };

    for instance in config.instances.iter_mut() {
        if instance.name.clone().unwrap().to_lowercase()
            == target_instance.clone().unwrap().unwrap().to_lowercase()
        {
            instance
                .installed_extensions
                .push(installed_extension.clone());
        }
    }

    match &config.write(&Config::full_path(args)) {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("there was an error: {}", e);
            bail!("there was an error writing the config: {e}")
        }
    }
}
