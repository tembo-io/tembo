use crate::cli::docker::DockerError;
use clap::ArgMatches;
use std::error::Error;

pub mod info;
pub mod login;

// handles all instance command calls
pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    if cfg!(target_os = "windows") {
        println!("{}", crate::WINDOWS_ERROR_MSG);

        return Err(Box::new(DockerError::new(crate::WINDOWS_ERROR_MSG)));
    }

    // execute the instance subcommands
    let res = match args.subcommand() {
        Some(("login", sub_matches)) => login::execute(sub_matches),
        Some(("info", sub_matches)) => info::execute(sub_matches),
        _ => unreachable!(),
    };

    if res.is_err() {
        println!("{}", res.err().unwrap());

        // TODO: adding logging, log error
        std::process::exit(101);
    }

    Ok(())
}
