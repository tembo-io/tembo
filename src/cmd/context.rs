use std::error::Error;

use clap::ArgMatches;
use simplelog::*;

pub mod list;
pub mod set;

// handles all context command calls
pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    // execute the context subcommands
    let res = match args.subcommand() {
        Some(("list", sub_matches)) => list::execute(sub_matches),
        Some(("set", sub_matches)) => set::execute(sub_matches),

        _ => unreachable!(),
    };

    if res.is_err() {
        error!("{}", res.err().unwrap());

        // TODO: adding logging, log error
        std::process::exit(101);
    }

    Ok(())
}
