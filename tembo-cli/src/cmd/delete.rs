use crate::{cli::docker::Docker, Result};
use clap::{ArgMatches, Command};

// Create init subcommand arguments
pub fn make_subcommand() -> Command {
    Command::new("delete").about("Deletes database instance locally & on tembo cloud")
}

pub fn execute(_args: &ArgMatches) -> Result<()> {
    Docker::stop_remove("tembo-pg")?;

    Ok(())
}
