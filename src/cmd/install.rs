use clap::{ArgMatches, Command};
use std::error::Error;

// Create clap subcommand arguments
pub fn make_subcommand() -> Command {
    Command::new("install")
        .about("Installs a local Tembo flavored version of Postgres")
        .arg(arg!(-s --stack "A Tembo stack type to install"))
}

pub fn execute(_args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    println!("coming soon");
    Ok(())
}
