#![feature(file_create_new)]

#[macro_use]
extern crate clap;
extern crate log;
extern crate serde;
extern crate serde_yaml;
extern crate simplelog;

use anyhow::anyhow;
use clap::{Arg, Command};
use clap_complete::Shell;
use serde::{Deserialize, Serialize};
use simplelog::*;
use std::fs::File;

mod cli;
mod cmd;

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
    if cfg!(target_os = "widows") {
        warn!("{}", crate::WINDOWS_ERROR_MSG);

        std::process::exit(101);
    }

    let command = create_clap_command();
    let matches = command.get_matches();

    let res = match matches.subcommand() {
        Some(("init", sub_matches)) => cmd::init::execute(sub_matches),
        Some(("instance", sub_matches)) => cmd::instance::execute(sub_matches),
        Some(("db", sub_matches)) => cmd::database::execute(sub_matches),
        Some(("schema", sub_matches)) => cmd::schema::execute(sub_matches),
        Some(("extension", sub_matches)) => cmd::extension::execute(sub_matches),
        Some(("auth", sub_matches)) => cmd::auth::execute(sub_matches),
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
        error!("{}", res.err().unwrap());

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
                .subcommand(cmd::instance::create::make_subcommand())
                .subcommand(cmd::instance::list::make_subcommand())
                .subcommand(cmd::instance::start::make_subcommand())
                .subcommand(cmd::instance::stop::make_subcommand()),
        )
        .subcommand(
            Command::new("auth")
                .about("Commands used to manage authentication")
                .subcommand(cmd::auth::login::make_subcommand())
                .subcommand(cmd::auth::info::make_subcommand()),
        )
        .subcommand(
            Command::new("db")
                .about("Commands used to manage local and cloud databases")
                .subcommand(cmd::database::create::make_subcommand()),
        )
        .subcommand(
            Command::new("schema")
                .about("Commands used to manage local and cloud schemas")
                .subcommand(cmd::schema::create::make_subcommand()),
        )
        .subcommand(
            Command::new("extension")
                .about("Commands used to manage local and cloud extensions")
                .subcommand(cmd::extension::list::make_subcommand())
                .subcommand(cmd::extension::install::make_subcommand()),
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
