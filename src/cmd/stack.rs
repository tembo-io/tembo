pub mod create {
    use crate::cli::config::{Config, EnabledExtensions, InstalledExtensions, Stacks};
    use crate::cli::docker::{Docker, DockerError};
    use crate::cli::stacks;
    use clap::{Arg, ArgAction, ArgMatches, Command};
    use spinners::{Spinner, Spinners};
    use std::error::Error;
    use std::process::Command as ShellCommand;

    pub fn make_subcommand() -> Command {
        Command::new("create")
            .about("Command used to create a local stack")
            .arg(
                Arg::new("stack")
                    .short('s')
                    .long("stack")
                    .action(ArgAction::Set)
                    .required(false)
                    .default_value("standard")
                    .help("The name of a Tembo stack type to install"),
            )
    }

    pub fn execute(args: &ArgMatches) -> Result<(), Box<dyn Error>> {
        if cfg!(target_os = "windows") {
            println!("{}", crate::WINDOWS_ERROR_MSG);

            return Err(Box::new(DockerError::new(crate::WINDOWS_ERROR_MSG)));
        }

        // NOTE: install is a command (so just use the args), stack create is a subcommand, so we
        // need to fetch the args
        let matches = if args.subcommand().is_none() {
            args
        } else {
            let (_name, matches) = args.subcommand().unwrap();
            matches
        };

        // ensure the stack type provided is valid, if none given, default to the standard stack
        if let Ok(stack) = stacks::define_stack(matches) {
            println!("- Preparing to install {} stack", stack);

            match check_requirements() {
                Ok(_) => println!("- Docker was found and appears running"),
                Err(e) => {
                    eprintln!("{}", e);
                    return Err(e);
                }
            }

            match build_image(&stack) {
                Ok(_) => println!("- Stack install started"),
                Err(e) => {
                    eprintln!("{}", e);
                    return Err(e);
                }
            }

            match install_stack_config(&stack, args) {
                Ok(_) => {
                    println!("- Stack configuration completed, extensions installed via Trunk")
                }
                Err(e) => {
                    eprintln!("{}", e);
                    return Err(e);
                }
            }

            println!(
                "- Stack install finished, you can start the stack using the command 'tembo start'"
            );
        } else {
            return Err(Box::new(stacks::StackError::new(
                "- Given Stack type is not valid",
            )));
        }

        Ok(())
    }

    fn check_requirements() -> Result<(), Box<dyn Error>> {
        Docker::installed_and_running()
    }

    fn install_stack_config(stack: &str, args: &ArgMatches) -> Result<(), Box<dyn Error>> {
        let stacks = stacks::define_stacks();
        let stack_details: Vec<_> = stacks
            .stacks
            .iter()
            .filter(|s| s.name.to_lowercase() == stack.to_lowercase())
            .collect();

        let desired_stack: &stacks::StackDetails = stack_details[0];

        for install in &desired_stack.trunk_installs {
            let _ = install_extension(stack, install);
        }

        for extension in &desired_stack.extensions {
            let _ = enable_extension(stack, extension);
        }

        let _ = persist_stack_config(desired_stack, args);

        Ok(())
    }

    fn install_extension(
        stack: &str,
        extension: &stacks::TrunkInstall,
    ) -> Result<(), Box<dyn Error>> {
        let mut sp = Spinner::new(Spinners::Dots12, "Installing extension".into());

        // TODO: init may need to move the dockerfile and docker-compose files to the
        // ~/.config/tembo directory
        let mut command = String::from("cd tembo && docker-compose ");
        command.push_str(stack);
        command.push_str(" run bash && trunk install ");
        command.push_str(&extension.name);

        let output = ShellCommand::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .expect("failed to execute process");

        let mut msg = String::from("- Stack extension installed: ");
        msg.push_str(&extension.name);

        sp.stop_with_message(msg);

        let stderr = String::from_utf8(output.stderr).unwrap();

        if !stderr.is_empty() {
            return Err(Box::new(DockerError::new(
                format!("There was an issue installing the extension: {}", stderr).as_str(),
            )));
        } else {
            // TODO: persist the extension info to the config

            Ok(())
        }
    }

    fn enable_extension(stack: &str, extension: &stacks::Extension) -> Result<(), Box<dyn Error>> {
        let mut sp = Spinner::new(Spinners::Dots12, "Enabling extension".into());

        let locations = extension
            .locations
            .iter()
            .map(|s| s.database.as_str())
            .collect::<Vec<&str>>()
            .join(", ");

        let mut command = String::from("docker compose run ");
        command.push_str(stack);
        command.push_str("psql -U postgres -c create extension if not exists \"");
        command.push_str(&extension.name);
        command.push_str("\" schema ");
        command.push_str(&locations);
        command.push_str(" cascade;");

        let output = ShellCommand::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .expect("failed to execute process");

        let mut msg = String::from("- Stack extension enabled: ");
        msg.push_str(&extension.name);

        sp.stop_with_message(msg);

        let stderr = String::from_utf8(output.stderr).unwrap();

        if !stderr.is_empty() {
            return Err(Box::new(DockerError::new(
                format!("There was an issue enabling the extension: {}", stderr).as_str(),
            )));
        } else {
            // TODO: persist what extensions are enabled in the config file
            //
            Ok(())
        }
    }

    fn build_image(stack: &str) -> Result<(), Box<dyn Error>> {
        if image_exist(stack) {
            println!("- The image already exists, proceeding");
            return Ok(());
        }

        let mut sp = Spinner::new(Spinners::Line, "Installing stack".into());
        let mut command = String::from("cd tembo");
        command.push_str("&& docker-compose build ");
        command.push_str(stack); // docker-compose contains service definitions for each stack
        command.push_str(" --no-cache --quiet");

        let output = ShellCommand::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .expect("failed to execute process");

        sp.stop_with_message("- Installing stack complete".into());

        let stderr = String::from_utf8(output.stderr).unwrap();

        if !stderr.is_empty() {
            return Err(Box::new(DockerError::new(
                format!("There was an issue building the container: {}", stderr).as_str(),
            )));
        } else {
            Ok(())
        }
    }

    fn image_exist(stack: &str) -> bool {
        let command = String::from("docker images");
        let output = ShellCommand::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .expect("failed to execute process");

        let stdout = String::from_utf8(output.stdout).unwrap();
        let mut image_name = String::from("tembo-");
        image_name.push_str(stack);
        let image = stdout.find(&image_name);

        image.is_some()
    }

    fn persist_stack_config(
        stack: &stacks::StackDetails,
        args: &ArgMatches,
    ) -> Result<(), Box<dyn Error>> {
        let mut config: Config = Config::new(args);
        let mut stack_config = Stacks {
            name: Some(stack.name.clone()),
            version: Some(stack.stack_version.clone()),
            installed_extensions: InstalledExtensions {
                name: None,
                version: None,
            },
            enabled_extensions: EnabledExtensions {
                name: None,
                version: None,
            },
        };

        for install in &stack.trunk_installs {
            stack_config.installed_extensions = InstalledExtensions {
                name: Some(install.name.clone()),
                version: Some(install.version.clone()),
            }
        }

        // TODO: don't overwrite the trunk installs, add or modify them
        for extension in &stack.extensions {
            stack_config.enabled_extensions = EnabledExtensions {
                name: Some(extension.name.clone()),
                version: Some(String::from("2.0")),
            }
        }

        config.stacks = stack_config;

        match Config::write(&config) {
            Ok(_) => println!("- Stack install info added to configuration file"),
            Err(e) => eprintln!("{}", e),
        }

        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use crate::cli::stacks::ExtensionLocation;

        use super::*;
        use crate::cli::stacks::*;
        use clap::{Arg, ArgAction, Command};

        // NOTE: need to mock check_requirements, build_image, install_stack_config
        #[test]
        #[ignore]
        fn valid_execute_test() {
            // with a valid stack type
            let stack_type = String::from("standard");

            let m = Command::new("myapp").subcommand(
                Command::new("create").arg(
                    Arg::new("stack")
                        .short('s')
                        .long("stack")
                        .action(ArgAction::Set)
                        .required(false)
                        .default_value("standard")
                        .help("The name of a Tembo stack type to install"),
                ),
            );

            let result =
                execute(&m.get_matches_from(vec!["myapp", "create", "--stack", &stack_type]));
            assert_eq!(result.is_ok(), true);
        }

        #[test]
        #[ignore]
        fn invalid_execute_test() {
            // with a valid stack type
            let stack_type = String::from("foo");

            let m = Command::new("myapp").subcommand(
                Command::new("create").arg(
                    Arg::new("stack")
                        .short('s')
                        .long("stack")
                        .action(ArgAction::Set)
                        .required(false)
                        .default_value("standard")
                        .help("The name of a Tembo stack type to install"),
                ),
            );

            let result =
                execute(&m.get_matches_from(vec!["myapp", "create", "--stack", &stack_type]));
            assert_eq!(result.is_err(), true);
        }

        #[test]
        #[ignore]
        fn persist_stack_config_test() {
            let stack_type = String::from("standard");
            let trunk_install = TrunkInstall {
                name: String::from("pgmq"),
                version: String::from("1.0"),
            };
            let extension_location = ExtensionLocation {
                database: String::from("my_warehouse"),
                enabled: String::from("true"),
                version: String::from("1.0"),
            };
            let extension = Extension {
                name: "pgmq".to_string(),
                locations: vec![extension_location],
            };
            let stack: StackDetails = StackDetails {
                name: String::from("foo"),
                description: String::from("some description"),
                stack_version: String::from("1.0"),
                trunk_installs: vec![trunk_install],
                extensions: vec![extension],
            };

            let m = Command::new("myapp").subcommand(
                Command::new("create").arg(
                    Arg::new("stack")
                        .short('s')
                        .long("stack")
                        .action(ArgAction::Set)
                        .required(false)
                        .default_value("standard")
                        .help("The name of a Tembo stack type to install"),
                ),
            );

            let matches = &m.get_matches_from(vec!["myapp", "create", "--stack", &stack_type]);
            let result = persist_stack_config(&stack, &matches);

            assert_eq!(result.is_ok(), true);

            // TODO: ensure the file contains the correct information
        }
    }
}
