#![feature(file_create_new)]

#[macro_use]
extern crate clap;
extern crate log;
extern crate serde;
extern crate serde_yaml;
extern crate simplelog;

use anyhow::Context;
use clap::{Arg, Command};
use clap_complete::Shell;
use serde::{Deserialize, Serialize};
use simplelog::*;
use std::fs::File;

mod cli;
mod cmd;

pub type Result<T = ()> = anyhow::Result<T>;

const VERSION: &str = concat!("v", crate_version!());
const WINDOWS_ERROR_MSG: &str = "Windows is not supported at this time";

fn main() {
    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Info,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Info,
            Config::default(),
            File::create("tembo.log").unwrap(),
        ),
    ])
    .unwrap();

    // Windows is not supported currently, will alert user and stop process immediately
    if cfg!(windows) {
        warn!("{}", crate::WINDOWS_ERROR_MSG);

        std::process::exit(101);
    }

    let command = create_clap_command();
    let matches = command.get_matches();

    let res = match matches.subcommand() {
        Some(("init", sub_matches)) => cmd::init::execute(sub_matches),
        Some(("context", sub_matches)) => cmd::context::execute(sub_matches),
        Some(("apply", sub_matches)) => cmd::apply::execute(sub_matches),
        Some(("delete", sub_matches)) => cmd::delete::execute(sub_matches),
        Some(("completions", sub_matches)) => (|| {
            let shell = sub_matches
                .get_one::<Shell>("shell")
                .with_context(|| "Shell name missing.")?;

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

    if let Err(err) = res {
        error!("{err}");

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
        .subcommand(cmd::apply::make_subcommand())
        .subcommand(cmd::delete::make_subcommand())
        .subcommand(
            Command::new("context")
                .about("Commands used to list/get/set context")
                .subcommand(cmd::context::list::make_subcommand())
                .subcommand(cmd::context::set::make_subcommand()),
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
