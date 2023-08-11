#![feature(file_create_new)]

#[macro_use]
extern crate clap;

use anyhow::anyhow;
use clap::{Arg, Command};
use clap_complete::Shell;

mod cmd;

const VERSION: &str = concat!("v", crate_version!());

fn main() {
    let command = create_clap_command();

    // Check which subcommand the user ran...
    let res = match command.get_matches().subcommand() {
        Some(("init", sub_matches)) => cmd::init::execute(sub_matches),
        Some(("install", sub_matches)) => cmd::install::execute(sub_matches),
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

    if let Err(_) = res {
        // TODO: adding logging, log error
        std::process::exit(101);
    }
}

/// Create a list of valid arguments and sub-commands
fn create_clap_command() -> Command {
    let app = Command::new(crate_name!())
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
        .subcommand(cmd::install::make_subcommand())
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
        );

    app
}
