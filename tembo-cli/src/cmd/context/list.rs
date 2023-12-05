use crate::Result;
use clap::{ArgMatches, Command};
use std::fs;

use crate::cli::context::{tembo_context_file_path, Context};

pub fn make_subcommand() -> Command {
    Command::new("list").about("Command used to list context")
}

pub fn execute(_args: &ArgMatches) -> Result<()> {
    let filename = tembo_context_file_path();

    let contents = match fs::read_to_string(&filename) {
        Ok(c) => c,
        Err(e) => {
            panic!("Couldn't read context file {}: {}", filename, e);
        }
    };

    let data: Context = match toml::from_str(&contents) {
        Ok(d) => d,
        Err(e) => {
            panic!("Unable to load data. Error: `{}`", e);
        }
    };

    // TODO: Improve formatting
    println!("Name           Target         Org ID         Profile         Set");
    println!("-------------- -------------- -------------- -------------- --------------");

    for e in data.environment {
        let mut org_id = String::from("           ");
        let mut profile = String::from("   ");
        let mut set = false;
        if let Some(env_org) = e.org_id {
            org_id = env_org;
        }
        if let Some(env_profile) = e.profile {
            profile = env_profile;
        }
        if let Some(env_set) = e.set {
            set = env_set;
        }
        println!(
            "{}           {}     {:?}      {:?}          {:?}",
            e.name, e.target, org_id, profile, set
        );
    }

    Ok(())
}
