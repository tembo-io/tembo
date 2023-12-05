use crate::Result;

use clap::ArgMatches;
use simplelog::*;

pub mod info;
pub mod login;

// handles all instance command calls
pub fn execute(args: &ArgMatches) -> Result<()> {
    // execute the instance subcommands
    let res = match args.subcommand() {
        Some(("login", sub_matches)) => login::execute(sub_matches),
        Some(("info", sub_matches)) => info::execute(sub_matches),
        _ => unreachable!(),
    };

    if res.is_err() {
        error!("{}", res.err().unwrap());

        // TODO: adding logging, log error
        std::process::exit(101);
    }

    Ok(())
}
