use crate::Result;
use clap::ArgMatches;
use simplelog::*;

pub mod create;
pub mod list;
pub mod start;

// handles all instance command calls
pub fn execute(args: &ArgMatches) -> Result<()> {
    // execute the instance subcommands
    let res = match args.subcommand() {
        Some(("create", sub_matches)) => create::execute(sub_matches),
        Some(("list", sub_matches)) => list::execute(sub_matches),
        Some(("start", sub_matches)) => start::execute(sub_matches),
        _ => unreachable!(),
    };

    if res.is_err() {
        error!("{}", res.err().unwrap());

        // TODO: adding logging, log error
        std::process::exit(101);
    }

    Ok(())
}
