#![feature(file_create_new)]

#[macro_use]
extern crate clap;
extern crate serde;
extern crate serde_yaml;

use anyhow::anyhow;
use clap::{Arg, Command};
use clap_complete::Shell;
use serde::{Deserialize, Serialize};

mod cli;
mod cmd;

const VERSION: &str = concat!("v", crate_version!());
const WINDOWS_ERROR_MSG: &str = "- Windows is not supported at this time";

fn main() {
    let command = create_clap_command();
    let matches = command.get_matches();

    let res = match matches.subcommand() {
        Some(("init", sub_matches)) => cmd::init::execute(sub_matches),
        Some(("instance", sub_matches)) => cmd::instance::create::execute(sub_matches),
        Some(("completions", sub_matches)) => (|| {
            let shell = sub_matches
                .get_one::<Shell>("shell")
                .ok_or_else(|| anyhow!("Shell name missing."))?;

            let mut complete_app = create_clap_command();
            clap_complete::generate(
                *shell,
                &mut complete_app,
                "tembo",
                &mut std::io::stdout().lock(),
            );
            Ok(())
        })(),
        _ => unreachable!(),
    };

    if res.is_err() {
        println!("{}", res.err().unwrap());

        // TODO: adding logging, log error
        std::process::exit(101);
    }
}

/// Create a list of valid arguments and sub-commands
fn create_clap_command() -> Command {
    Command::new(crate_name!())
        .about(crate_description!())
        .author("Tembo <ry@tembo.io>")
        .version(VERSION)
        .propagate_version(true)
        .arg_required_else_help(true)
        .after_help(
            "For more information about a specific command, try `tembo <command> --help`\n\
             The source code for tembo is available at: https://github.com/tembo-io/tembo-cli",
        )
        .subcommand(cmd::init::make_subcommand())
        .subcommand(
            Command::new("instance")
                .about("Commands used to manage local and cloud instances")
                .subcommand(cmd::instance::create::make_subcommand()),
        )
        .subcommand(
            Command::new("completions")
                .about("Generate shell completions for your shell to stdout")
                .arg(
                    Arg::new("shell")
                        .value_parser(clap::value_parser!(Shell))
                        .help("the shell to generate completions for")
                        .value_name("SHELL")
                        .required(true),
                ),
        )
}
