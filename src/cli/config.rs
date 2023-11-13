#![allow(dead_code)]

use crate::cli::cloud_account::CloudAccount;
use crate::cli::instance::Instance;
use chrono::prelude::*;
use clap::ArgMatches;
use serde::Deserialize;
use serde::Serialize;
use simplelog::*;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::path::PathBuf;

const CONFIG_FILE_NAME: &str = "configuration.toml";
const CONFIG_FILE_PATH: &str = ".config/tembo/";

// TODO: look into swapping this out for https://crates.io/crates/config

// NOTE: modifying the struct determines what gets persisted in the configuration file
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub created_at: DateTime<Utc>,
    pub cloud_account: Option<CloudAccount>,
    pub jwt: Option<String>,
    pub instances: Vec<Instance>,
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let config_str = toml::to_string(&self).unwrap();

        f.write_str(config_str.as_ref())
    }
}

impl Config {
    // Returns a default Rust object that will be persisted as serialized toml
    pub fn new(_args: &ArgMatches, file_path: &PathBuf) -> Config {
        let utc: DateTime<Utc> = Utc::now();

        match Self::read_to_string(file_path) {
            Ok(contents) => Self::to_toml(&contents),
            Err(_) => {
                // NOTE: the defaults that get written to the configuration file
                let config = Config {
                    created_at: utc,
                    cloud_account: None,
                    jwt: None,
                    instances: vec![],
                };

                let _init = Self::init(&config, file_path);
                let _write = Self::write(&config, file_path);

                config
            }
        }
    }

    // Reads the contents of an existing config file and returns contents as a string
    pub fn read_to_string(file_path: &PathBuf) -> Result<String, Box<dyn Error>> {
        let mut file = File::open(file_path)?;
        let mut contents = String::new();

        file.read_to_string(&mut contents)
            .expect("Unable to read stack config file");

        Ok(contents)
    }

    // Returns a Config object serialized to toml from a string
    pub fn to_toml(str: &str) -> Config {
        let config: Config = toml::from_str(str).unwrap();

        config
    }

    // Writes the current Config to the config file, overwriting anything else that was there
    pub fn write(&self, file_path: &PathBuf) -> Result<(), Box<dyn Error>> {
        let mut file = File::create(file_path)?;
        let _delete = file.set_len(0); // this deletes all contents from the file

        let _result = file.write_all(self.to_string().as_bytes());

        Ok(())
    }

    // Returns the full path to the config file
    pub fn full_path(_args: &ArgMatches) -> PathBuf {
        // NOTE: only supporting a file in the home directory for now
        let home_dir = home::home_dir();

        // if home directory can not be determined, use the current directory
        match home_dir {
            Some(mut path) => {
                path.push(CONFIG_FILE_PATH);
                path.push(CONFIG_FILE_NAME);

                path
            }
            None => env::current_dir().expect("Unable to determine the current directory"),
        }
    }

    // Creates the config directory
    fn create_config_dir(dir_path: &str) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(dir_path)?;

        Ok(())
    }

    // Creates the config file in the config directory
    fn create_config_file(path: &str) -> Result<(), Box<dyn Error>> {
        File::create_new(path)?; // don't overwrite existing file at path

        Ok(())
    }

    // Initializes the config file, creating the directories and files as needed
    fn init(&self, file_path: &Path) -> Result<(), Box<dyn Error>> {
        let mut dir_path = file_path.to_path_buf();
        dir_path.pop(); // removes any filename and extension

        match Config::create_config_dir(&dir_path.to_string_lossy()) {
            Ok(()) => Config::create_config_file(&file_path.to_string_lossy()),
            Err(e) => {
                error!("Directory can not be created, {}", e);

                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::config::Config;
    use crate::cli::instance::{EnabledExtension, InstalledExtension, Instance};
    use clap::{Arg, ArgAction, Command};
    use std::env;

    fn test_path() -> PathBuf {
        let mut path: PathBuf = env::current_dir().unwrap();
        path.push("tests");
        path.push(".config");
        path.push("tembo");

        let _result = Config::create_config_dir(&path.to_string_lossy());

        path.push("configuration.toml");

        return path;
    }

    fn setup() -> Config {
        let matches = Command::new("myapp")
            .arg(
                Arg::new("file-path")
                    .value_parser(clap::value_parser!(std::path::PathBuf))
                    .action(ArgAction::Set)
                    .required(false),
            )
            .get_matches_from(vec!["myapp"]);

        let path: PathBuf = test_path();
        let config = Config::new(&matches, &path); // calls init and writes the file

        return config;
    }

    fn cleanup() {
        let path = test_path();
        let _ = std::fs::remove_file(&*path.to_string_lossy());
    }

    // NOTE: wrap tests that require a setup and cleanup step
    #[test]
    fn config_tests() {
        setup();

        init_test();
        read_to_string_test();
        to_toml_test();

        cleanup();
    }

    fn init_test() {
        // retrieves the full path, pops off the file_path, creates the directories if needed, and
        // writes the file
        let matches = Command::new("myapp")
            .arg(
                Arg::new("file-path")
                    .value_parser(clap::value_parser!(std::path::PathBuf))
                    .action(ArgAction::Set)
                    .required(false),
            )
            .get_matches_from(vec!["myapp"]);

        let path = test_path();
        let _config = Config::new(&matches, &path); // calls init and writes the file
                                                    //
        let file = File::open(path);
        let mut contents = String::new();

        let _ = file.unwrap().read_to_string(&mut contents);

        assert_eq!(contents, Config::to_toml(&contents).to_string());
    }

    fn read_to_string_test() {
        let _matches = Command::new("myapp")
            .arg(
                Arg::new("file-path")
                    .value_parser(clap::value_parser!(std::path::PathBuf))
                    .action(ArgAction::Set)
                    .required(false),
            )
            .get_matches_from(vec!["myapp"]);

        let path = test_path();
        let config = Config::read_to_string(&path);

        assert_eq!(config.is_ok(), true);
    }

    fn to_toml_test() {
        let mut config = setup();
        let toml = Config::to_toml(&config.to_string());

        // with no instances
        assert_eq!(toml.instances, vec![]);

        // wth instances
        let instance = Instance {
            name: Some(String::from("instance_name")),
            r#type: Some(String::from("standard")),
            port: Some(String::from("5432")),
            version: Some(String::from("1.1")),
            created_at: Some(Utc::now()),
            installed_extensions: vec![InstalledExtension {
                name: Some(String::from("pgmq")),
                version: Some(String::from("1.0")),
                created_at: Some(Utc::now()),
            }],
            enabled_extensions: vec![EnabledExtension {
                name: Some(String::from("pgmq")),
                version: Some(String::from("1.0")),
                created_at: Some(Utc::now()),
                locations: vec![],
            }],
            databases: vec![],
        };
        config.instances = vec![instance];

        let toml = Config::to_toml(&config.to_string());

        //assert_eq!(toml.created_at.is_some(), true);
        assert_eq!(toml.instances[0].name, Some(String::from("instance_name")));
        assert_eq!(
            toml.instances[0].installed_extensions[0].name,
            Some(String::from("pgmq"))
        );
    }
    /*
    #[test]
    fn full_path_test() {
        let matches = Command::new("myapp")
            .arg(
                Arg::new("file-path")
                    .value_parser(clap::value_parser!(std::path::PathBuf))
                    .action(ArgAction::Set)
                    .required(false),
            )
            .get_matches_from(vec!["myapp"]);

        let binding = home::home_dir().unwrap();
        let home_dir = &binding.to_str().unwrap();

        let result = Config::full_path(&matches);

        assert!(result.to_str().unwrap().contains(&*home_dir));
    }

    #[test]
    fn create_config_dir_test() {
        let mut path: PathBuf = test_path();
        path.pop();

        let write = Config::create_config_dir(&path.to_string_lossy());
        assert_eq!(write.is_ok(), true);

        let overwrite = Config::create_config_file(&path.to_string_lossy());
        assert_eq!(overwrite.is_err(), true);

        cleanup();
    }

    #[test]
    fn create_config_file_test() {
        let path: PathBuf = test_path();

        let write = Config::create_config_file(&path.to_string_lossy());
        assert_eq!(write.is_ok(), true);

        let overwrite = Config::create_config_file(&path.to_string_lossy());
        assert_eq!(overwrite.is_err(), true);

        cleanup();
    }
    */
}
