use crate::{cli::context::list_context, Result};
use clap::{ArgMatches, Command};

pub fn make_subcommand() -> Command {
    Command::new("list").about("Command used to list context")
}

pub fn execute(_args: &ArgMatches) -> Result<()> {
    let context = list_context()?;

    // TODO: Improve formatting
    println!("Name           Target         Org ID         Profile         Set");
    println!("-------------- -------------- -------------- -------------- --------------");

    for e in context.environment {
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
