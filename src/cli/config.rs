#![allow(dead_code)]

use clap::ArgMatches;
use serde::Deserialize;
use serde::Serialize;
use std::cmp::PartialEq;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

const CONFIG_FILE_NAME: &str = "configuration.toml";

// NOTE: modifying the struct determines what gets persisted in the configuration file
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub file_name: String,
    pub file_path: PathBuf, // NOTE: may support additional file paths in the future (ie. for an individual project or having multiple accounts for example)
    pub stacks: Stacks,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Stacks {
    pub name: Option<String>,
    pub version: Option<String>,
    pub installed_extensions: InstalledExtensions,
    pub enabled_extensions: EnabledExtensions,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct InstalledExtensions {
    pub name: Option<String>,
    pub version: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct EnabledExtensions {
    pub name: Option<String>,
    pub version: Option<String>,
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let config_str = toml::to_string(&self).unwrap();

        f.write_str(config_str.as_ref())
    }
}

impl Config {
    // Returns a default Rust object that will be persisted as serialized toml
    pub fn new(args: &ArgMatches) -> Config {
        match Self::read_to_string(args) {
            Ok(contents) => Self::to_toml(&contents),
            Err(_) => {
                let config = Config {
                    file_name: CONFIG_FILE_NAME.to_string(),
                    file_path: Self::full_path(args),
                    stacks: Stacks {
                        name: None,
                        version: None,
                        installed_extensions: InstalledExtensions {
                            name: None,
                            version: None,
                        },
                        enabled_extensions: EnabledExtensions {
                            name: None,
                            version: None,
                        },
                    },
                };

                let _ = Self::init(&config);
                let _ = Self::write(&config);

                config
            }
        }
    }

    // Reads the contents of any existing config file and returns contents as a string
    pub fn read_to_string(args: &ArgMatches) -> Result<String, Box<dyn Error>> {
        let file = Self::full_path(args);
        let mut file = File::open(file)?;
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
    pub fn write(&self) -> Result<(), Box<dyn Error>> {
        let file_path = self.file_path.clone();

        let mut file = File::create(file_path)?;
        let _delete = file.set_len(0); // this deletes all contents from the file
        let _result = file.write_all(self.to_string().as_bytes());

        Ok(())
    }

    // Creates the config directory
    fn create_config_dir(dir_path: &str) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(dir_path)?;

        Ok(())
    }

    // Creates the config file in the config directory
    fn create_config_file(path: String) -> Result<(), Box<dyn Error>> {
        File::create_new(path)?; // don't overwrite existing file at path

        Ok(())
    }

    // Returns the full path to the config file
    fn full_path(_args: &ArgMatches) -> PathBuf {
        // NOTE: only supporting a file in the home directory for now
        let home_dir = home::home_dir();

        // if home directory can not be determined, use the current directory
        match home_dir {
            Some(mut path) => {
                path.push(".config");
                path.push("tembo");
                path.push(CONFIG_FILE_NAME);

                path
            }
            None => env::current_dir().expect("Unable to determine the current directory"),
        }
    }

    // Initializes the config file, creating the directories and files as needed
    fn init(&self) -> Result<(), Box<dyn Error>> {
        let mut full_path = self.file_path.clone();
        full_path.pop(); // removes any filename and extension

        match Config::create_config_dir(&full_path.to_string_lossy()) {
            Ok(()) => Config::create_config_file(self.file_path.to_string_lossy().into_owned()),
            Err(e) => {
                println!("- Directory can not be created, {}", e);

                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::config::Config;
    use clap::{Arg, ArgAction, Command};
    use std::env;
    use std::io::Read;

    // NOTE: many of the tests below assume the config file is in place, this is a convenient
    // helper to create the file if it doesn't already exist
    fn setup() {
        let matches = Command::new("myapp")
            .arg(
                Arg::new("file-path")
                    .value_parser(clap::value_parser!(std::path::PathBuf))
                    .action(ArgAction::Set)
                    .required(false),
            )
            .get_matches_from(vec!["myapp"]);

        let _config = Config::new(&matches); // calls init and writes the file
    }

    #[test]
    fn read_to_string_test() {
        setup();

        let matches = Command::new("myapp")
            .arg(
                Arg::new("file-path")
                    .value_parser(clap::value_parser!(std::path::PathBuf))
                    .action(ArgAction::Set)
                    .required(false),
            )
            .get_matches_from(vec!["myapp"]);

        let config = Config::read_to_string(&matches);

        assert_eq!(config.is_ok(), true);
    }

    #[test]
    fn to_toml_test() {
        let matches = Command::new("myapp")
            .arg(
                Arg::new("file-path")
                    .value_parser(clap::value_parser!(std::path::PathBuf))
                    .action(ArgAction::Set)
                    .required(false),
            )
            .get_matches_from(vec!["myapp"]);

        // defaults
        let config = Config::new(&matches);
        let toml = Config::to_toml(&config.to_string());

        assert_eq!(toml.file_name, CONFIG_FILE_NAME);
        assert_eq!(
            toml.stacks,
            Stacks {
                name: None,
                version: None,
                installed_extensions: InstalledExtensions {
                    name: None,
                    version: None,
                },
                enabled_extensions: EnabledExtensions {
                    name: None,
                    version: None,
                }
            }
        );

        // wth stacks
        let mut config = Config::new(&matches);
        config.stacks = Stacks {
            name: Some(String::from("stack_name")),
            version: Some(String::from("1.1")),
            installed_extensions: InstalledExtensions {
                name: Some(String::from("pgmq")),
                version: Some(String::from("1.0")),
            },
            enabled_extensions: EnabledExtensions {
                name: Some(String::from("pgmq")),
                version: Some(String::from("1.0")),
            },
        };

        let toml = Config::to_toml(&config.to_string());

        assert_eq!(toml.file_name, CONFIG_FILE_NAME);
        assert_eq!(
            toml.stacks,
            Stacks {
                name: Some(String::from("stack_name")),
                version: Some(String::from("1.1")),
                installed_extensions: InstalledExtensions {
                    name: Some(String::from("pgmq")),
                    version: Some(String::from("1.0")),
                },
                enabled_extensions: EnabledExtensions {
                    name: Some(String::from("pgmq")),
                    version: Some(String::from("1.0")),
                }
            }
        );
    }

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

        assert!(Config::full_path(&matches)
            .to_str()
            .unwrap()
            .contains(&*home_dir));
    }

    #[test]
    fn create_config_dir_test() {
        let mut path: PathBuf = env::current_dir().unwrap();
        path.push("tests");
        path.push(".config");

        let write = Config::create_config_dir(&path.to_string_lossy().into_owned());
        assert_eq!(write.is_ok(), true);

        let overwrite = Config::create_config_file(path.to_string_lossy().into_owned());
        assert_eq!(overwrite.is_err(), true);

        // clean up
        let _ = std::fs::remove_dir(&*path.to_string_lossy());
    }

    #[test]
    fn create_config_file_test() {
        let mut path: PathBuf = env::current_dir().unwrap();
        path.push("tests");
        path.push("configuration.toml");

        let write = Config::create_config_file(path.to_string_lossy().into_owned());
        assert_eq!(write.is_ok(), true);

        let overwrite = Config::create_config_file(path.to_string_lossy().into_owned());
        assert_eq!(overwrite.is_err(), true);

        // clean up
        let _ = std::fs::remove_file(&*path.to_string_lossy());
    }

    #[test]
    fn init_test() {
        setup();
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

        let _config = Config::new(&matches); // calls init and writes the file
        let path = Config::full_path(&matches);

        let file = File::open(path);
        let mut contents = String::new();

        let _ = file.unwrap().read_to_string(&mut contents);

        assert_eq!(contents, Config::to_toml(&contents).to_string());

        // clean up
        let _ = std::fs::remove_file(Config::full_path(&matches));
    }
}
