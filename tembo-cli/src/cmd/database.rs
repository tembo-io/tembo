use crate::Result;
use clap::ArgMatches;
use simplelog::*;

pub mod create;

// handles all extension command calls
pub fn execute(args: &ArgMatches) -> Result<()> {
    // execute the instance subcommands
    let res = match args.subcommand() {
        Some(("create", sub_matches)) => create::execute(sub_matches),
        _ => unreachable!(),
    };

    if let Err(err) = res {
        error!("{err}");

        std::process::exit(101);
    }

    Ok(())
}
