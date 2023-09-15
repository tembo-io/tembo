// instance list command
use crate::cli::config::Config;
use crate::cli::docker::DockerError;
use clap::{ArgMatches, Command};
use std::error::Error;

// example usage: tembo instance create -t oltp -n my_app_db -p 5432
pub fn make_subcommand() -> Command {
    Command::new("list").about("Command used to list local instances")
}

pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    if cfg!(target_os = "windows") {
        println!("{}", crate::WINDOWS_ERROR_MSG);

        return Err(Box::new(DockerError::new(crate::WINDOWS_ERROR_MSG)));
    }

    let config = Config::new(args, &Config::full_path(args));

    if config.instances.is_empty() {
        println!("- No instances have been configured");
    } else {
        println!("- Listing of configured instances");

        for instance in &config.instances {
            println!(
                "      {} - type: {}, port: {}",
                instance.name.clone().unwrap(),
                instance.r#type.clone().unwrap(),
                instance.port.clone().unwrap()
            );
        }

        println!("- Start an instance using `tembo instance start -n <name>`");
        println!("- Coming soon: deploy an instance using `tembo instance deploy -n <name>`");
    }

    Ok(())
}
