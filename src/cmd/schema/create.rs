//  database create command
use crate::cli::config::Config;
use crate::cli::instance::Instance;
use crate::cli::instance::InstanceError;
use crate::cli::schema::Schema;
use chrono::Utc;
use clap::{Arg, ArgAction, ArgMatches, Command};
use simplelog::*;
use spinners::{Spinner, Spinners};
use std::error::Error;
use std::process::Command as ShellCommand;

// example usage: tembo schema create -n my_schema -d my_database -i my_instance
pub fn make_subcommand() -> Command {
    Command::new("create")
        .about("Command used to create databases on instances")
        .arg(
            Arg::new("name")
                .short('n')
                .long("name")
                .action(ArgAction::Set)
                .required(true)
                .help("The name of the schema to create"),
        )
        .arg(
            Arg::new("database")
                .short('d')
                .long("database")
                .action(ArgAction::Set)
                .required(true)
                .help("The name of the database to associate the schema with"),
        )
        .arg(
            Arg::new("instance")
                .short('i')
                .long("instance")
                .action(ArgAction::Set)
                .required(true)
                .help("The name of the instance to create the schema on"),
        )
}

pub fn execute(args: &ArgMatches) -> Result<(), Box<InstanceError>> {
    let config = Config::new(args, &Config::full_path(args));
    let name_arg = args.try_get_one::<String>("name").unwrap();
    let database_arg = args.try_get_one::<String>("database").unwrap();
    let instance_arg = args.try_get_one::<String>("instance").unwrap();

    info!(
        "trying to create schema '{}' for database '{}' on instance '{}'",
        &name_arg.unwrap(),
        &database_arg.unwrap(),
        &instance_arg.unwrap()
    );

    if config.instances.is_empty() {
        warn!("- No instances have been configured");
    } else {
        let _ = match Instance::find(args, instance_arg.unwrap()) {
            Ok(instance) => create_schema(instance, name_arg.unwrap(), args),
            Err(e) => Err(Box::new(e)),
        };
    }

    Ok(())
}

fn create_schema(
    instance: Instance,
    name: &str,
    args: &ArgMatches,
) -> Result<(), Box<InstanceError>> {
    instance.start();

    let mut sp = Spinner::new(Spinners::Dots12, "Creating schema".into());

    // psql -h localhost -U postgres -c 'create schema if not exists test;'
    let mut command = String::from("psql -h localhost -U postgres -p ");
    command.push_str(&instance.port.clone().unwrap());
    command.push_str(" -c 'create schema if not exists ");
    command.push_str(name);
    command.push_str(";'");

    let output = ShellCommand::new("sh")
        .arg("-c")
        .arg(&command)
        .output()
        .expect("failed to execute process");

    sp.stop_with_newline();

    let stderr = String::from_utf8(output.stderr).unwrap();

    if !stderr.is_empty() {
        Err(Box::new(InstanceError {
            name: format!("There was an issue creating the schema: {}", stderr),
        }))
    } else {
        info!("schema created");

        let _ = persist_config(args, instance);

        Ok(())
    }
}

fn persist_config(args: &ArgMatches, target_instance: Instance) -> Result<(), Box<dyn Error>> {
    let mut config = Config::new(args, &Config::full_path(args));
    let name_arg = args.try_get_one::<String>("name");
    let database_arg = args.try_get_one::<String>("database");

    // loop through instances
    for instance in config.instances.iter_mut() {
        if instance.name.clone().unwrap().to_lowercase()
            == target_instance.name.clone().unwrap().to_lowercase()
        {
            // loop through instance's databases
            for database in instance.databases.iter_mut() {
                if database.name.clone().to_lowercase()
                    == database_arg.clone().unwrap().unwrap().to_lowercase()
                {
                    let schema = Schema {
                        name: name_arg.clone().unwrap().unwrap().to_string(),
                        created_at: Utc::now(),
                    };

                    database.schemas.push(schema.clone());
                }
            }
        }
    }

    match &config.write(&Config::full_path(args)) {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("there was an error: {}", e);
            Err("there was an error writing the config".into())
        }
    }
}
