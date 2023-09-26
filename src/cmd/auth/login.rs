// auth login command
// use crate::cli::config::Config;
use crate::cli::docker::DockerError;
use crate::cli::{auth_client::AuthClient, config::Config};
use clap::{ArgMatches, Command};
use std::error::Error;

// example usage: tembo auth login
pub fn make_subcommand() -> Command {
    Command::new("login").about("Command used to login/authenticate")
}

pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    if cfg!(target_os = "windows") {
        println!("{}", crate::WINDOWS_ERROR_MSG);

        return Err(Box::new(DockerError::new(crate::WINDOWS_ERROR_MSG)));
    }

    match AuthClient::authenticate() {
        Ok(jwt) => {
            println!("- storing jwt in config file, it will be used in future requests");

            let mut config = Config::new(args, &Config::full_path(args));
            config.jwt = Some(jwt);
            let _ = config.write(&Config::full_path(args));

            Ok(())
        }
        Err(e) => {
            println!("- there was an error authenticating");
            Err(e)
        }
    }
}
