use crate::cli::context::{tembo_context_file_path, Context};
use clap::{Arg, ArgAction, ArgMatches, Command};
use std::io::Write;
use std::{
    error::Error,
    fs::{self, File},
};
use toml::to_string;

pub fn make_subcommand() -> Command {
    Command::new("set")
        .arg(
            Arg::new("name")
                .short('n')
                .long("name")
                .action(ArgAction::Set)
                .required(true)
                .help("The name of the context to set"),
        )
        .about("Command used to set context")
}

pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    let filename = tembo_context_file_path();

    let contents = match fs::read_to_string(&filename) {
        Ok(c) => c,
        Err(e) => {
            panic!("Couldn't read context file {}: {}", filename, e);
        }
    };

    let mut data: Context = match toml::from_str(&contents) {
        Ok(d) => d,
        Err(e) => {
            panic!("Unable to load data. Error: `{}`", e);
        }
    };

    let name = args.get_one::<String>("name").unwrap();

    for e in data.environment.iter_mut() {
        if &e.name == name {
            e.set = Some(true)
        } else {
            e.set = None
        }
    }

    if let Err(e) = write_config_to_file(&data, &tembo_context_file_path()) {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

fn write_config_to_file(
    config: &Context,
    file_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let toml_string = to_string(config)?;
    let mut file = File::create(file_path)?;

    file.write_all(toml_string.as_bytes())?;

    Ok(())
}
