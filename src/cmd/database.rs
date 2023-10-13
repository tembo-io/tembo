use clap::ArgMatches;
use simplelog::*;
use std::error::Error;

pub mod create;

// handles all extension command calls
pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    // execute the instance subcommands
    let res = match args.subcommand() {
        Some(("create", sub_matches)) => create::execute(sub_matches),
        _ => unreachable!(),
    };

    if res.is_err() {
        error!("{}", res.err().unwrap().name);

        std::process::exit(101);
    }

    Ok(())
}
