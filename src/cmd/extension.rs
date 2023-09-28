use crate::cli::docker::DockerError;
use clap::ArgMatches;
use simplelog::*;
use std::error::Error;

pub mod list;

// handles all extension command calls
pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    if cfg!(target_os = "windows") {
        warn!("{}", crate::WINDOWS_ERROR_MSG);

        return Err(Box::new(DockerError::new(crate::WINDOWS_ERROR_MSG)));
    }

    // execute the instance subcommands
    let res = match args.subcommand() {
        Some(("list", sub_matches)) => list::execute(sub_matches),
        _ => unreachable!(),
    };

    if res.is_err() {
        error!("{}", res.err().unwrap());

        // TODO: adding logging, log error
        std::process::exit(101);
    }

    Ok(())
}
