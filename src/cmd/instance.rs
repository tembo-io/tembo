use crate::cli::docker::DockerError;
use clap::ArgMatches;
use std::error::Error;

pub mod create;
pub mod list;
pub mod start;
pub mod stop;

// handles all instance command calls
pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    if cfg!(target_os = "windows") {
        println!("{}", crate::WINDOWS_ERROR_MSG);

        return Err(Box::new(DockerError::new(crate::WINDOWS_ERROR_MSG)));
    }

    // execute the instance subcommands
    let res = match args.subcommand() {
        Some(("create", sub_matches)) => create::execute(sub_matches),
        Some(("list", sub_matches)) => list::execute(sub_matches),
        Some(("start", sub_matches)) => start::execute(sub_matches),
        Some(("stop", sub_matches)) => stop::execute(sub_matches),
        _ => unreachable!(),
    };

    if res.is_err() {
        println!("{}", res.err().unwrap());

        // TODO: adding logging, log error
        std::process::exit(101);
    }

    Ok(())
}
