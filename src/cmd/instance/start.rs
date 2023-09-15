// instance start command
use crate::cli::config::Config;
use crate::cli::docker::DockerError;
use anyhow::anyhow;
use clap::{Arg, ArgAction, ArgMatches, Command};
use spinners::{Spinner, Spinners};
use std::error::Error;
use std::process::Command as ShellCommand;

// example usage: tembo instance start -n my_app_db
pub fn make_subcommand() -> Command {
    Command::new("start")
        .about("Command used to start local instances")
        .arg(
            Arg::new("name")
                .short('n')
                .long("name")
                .action(ArgAction::Set)
                .required(true)
                .help("The name you want to use for this instance"),
        )
}

pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    if cfg!(target_os = "windows") {
        println!("{}", crate::WINDOWS_ERROR_MSG);

        return Err(Box::new(DockerError::new(crate::WINDOWS_ERROR_MSG)));
    }

    let config = Config::new(args, &Config::full_path(args));
    let name = args
        .get_one::<String>("name")
        .ok_or_else(|| anyhow!("Name is missing."))?;

    if config.instances.is_empty() {
        println!("- No instances have been configured");
    } else {
        println!("- Finding config for {}", name);

        for instance in &config.instances {
            if instance.name.clone().unwrap().to_lowercase() == name.to_lowercase() {
                println!("config has been found");
                println!("starting via Docker");

                let mut sp = Spinner::new(Spinners::Line, "Starting instance".into());
                let port_option = format!(
                    "--publish {}:{}",
                    &instance.port.clone().unwrap(),
                    &instance.port.clone().unwrap(),
                );
                let mut command = String::from("cd tembo "); // TODO: does this work for installed crates?
                command.push_str("&& docker run -d --name ");
                command.push_str(&instance.name.clone().unwrap());
                command.push(' ');
                command.push_str(&port_option);
                command.push_str(" tembo-pg");

                let output = ShellCommand::new("sh")
                    .arg("-c")
                    .arg(&command)
                    .output()
                    .expect("failed to execute process");

                let message = format!(
                    "- Tembo instance started on {}",
                    &instance.port.clone().unwrap(),
                );
                sp.stop_with_message(message);

                let stderr = String::from_utf8(output.stderr).unwrap();

                if !stderr.is_empty() {
                    return Err(Box::new(DockerError::new(
                        format!("There was an issue starting the instance: {}", stderr).as_str(),
                    )));
                }
            }
        }
    }

    Ok(())
}
