use clap::builder::NonEmptyStringValueParser;
use clap::{Arg, ArgAction, ArgMatches, Command};
use std::error::Error;

// Create clap subcommand arguments
pub fn make_subcommand() -> Command {
    Command::new("init")
        .about("Initializes a local environment or project; generates configuration")
        .arg(
            Arg::new("dry-run")
                .short('d')
                .long("dry run")
                .value_name("dry-run"),
        )
        .arg(
            Arg::new("file-path")
                .short('f')
                .long("file-path")
                .value_name("dir")
                .value_parser(NonEmptyStringValueParser::new())
                .action(ArgAction::Append)
                .help(
                    "A path to the directory to add to the configuration \
                    file to, default is $HOME/.config/tembo",
                ),
        )
}

pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    let _dry_run: Option<&str> = args.get_one::<String>("dry-run").map(|s| s.as_str());
    let _file_path: Option<&str> = args.get_one::<String>("file-path").map(|s| s.as_str());

    // TODO: implement init command to generate a config file
    println!("Soon this will persist a configuration file");

    Ok(())
}
