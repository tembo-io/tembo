use crate::Result;
use clap::ArgMatches;
use simplelog::*;

pub mod install;
pub mod list;

// handles all extension command calls
pub fn execute(args: &ArgMatches) -> Result<()> {
    // execute the instance subcommands
    let res = match args.subcommand() {
        Some(("list", sub_matches)) => list::execute(sub_matches),
        Some(("install", sub_matches)) => install::execute(sub_matches),
        _ => unreachable!(),
    };

    if res.is_err() {
        error!("{}", res.err().unwrap());

        // TODO: adding logging, log error
        std::process::exit(101);
    }

    Ok(())
}
