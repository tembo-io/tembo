// extension list command
use crate::cli::config::Config;
use crate::cli::docker::DockerError;
use crate::cli::instance::Instance;
use clap::{Arg, ArgAction, ArgMatches, Command};
use std::error::Error;

// example usage: tembo extension list -n test
pub fn make_subcommand() -> Command {
    Command::new("list")
        .about("Command used to list extensions for instances")
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
        println!("{}", crate::WINDOWS_ERROR_MSG);

        return Err(Box::new(DockerError::new(crate::WINDOWS_ERROR_MSG)));
    }

    let config = Config::new(args, &Config::full_path(args));
    let name = args.try_get_one::<String>("name").unwrap();

    if config.instances.is_empty() {
        println!("- No instances have been configured");
    } else {
        let mut instances = vec![];

        for instance in &config.instances {
            if instance.name.clone().unwrap().to_lowercase() == name.unwrap().to_lowercase() {
                instances.push(instance);

                installed_extensions(instance);
                enabled_extensions(instance);
            }
        }

        if instances.is_empty() {
            println!("- No configuration found for {}", &name.unwrap());
        }
    }

    Ok(())
}

fn installed_extensions(instance: &Instance) {
    println!("- Installed extensions");

    for extension in &instance.installed_extensions {
        println!(
            "      {} - version: {}, installed: {}",
            extension.name.clone().unwrap(),
            extension.version.clone().unwrap(),
            extension.created_at.unwrap().clone()
        );
    }
}

fn enabled_extensions(instance: &Instance) {
    println!("- Enabled extensions (on databases)");

    let mut extensions = vec![];

    for extension in &instance.enabled_extensions {
        let mut locations = vec![];

        for location in &extension.locations {
            if location.enabled == "true" {
                locations.push(location.database.clone());
            }
        }

        if !locations.is_empty() {
            extensions.push(extension);

            println!(
                "      {} - locations: {}",
                extension.name.clone().unwrap(),
                locations.join(","),
            );
        }
    }

    if extensions.is_empty() {
        println!("      none");
    }
}
