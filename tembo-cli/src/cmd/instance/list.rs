//! instance list command

use crate::cli::config::Config;
use crate::Result;
use clap::{ArgMatches, Command};
use simplelog::*;

// example usage: tembo instance create -t oltp -n my_app_db -p 5432
pub fn make_subcommand() -> Command {
    Command::new("list").about("Command used to list local instances")
}

pub fn execute(args: &ArgMatches) -> Result<()> {
    let config = Config::new(args, &Config::full_path(args));

    if config.instances.is_empty() {
        info!("No instances have been configured");
    } else {
        info!("Listing of configured instances");

        for instance in &config.instances {
            info!(
                "    {} - type: {}, port: {}",
                instance.name.clone().unwrap(),
                instance.r#type.clone().unwrap(),
                instance.port.clone().unwrap()
            );
        }

        info!("Start an instance using `tembo instance start -n <name>`");
        info!("Coming soon: deploy an instance using `tembo instance deploy -n <name>`");
    }

    Ok(())
}
