use crate::{Deserialize, Serialize};
use clap::ArgMatches;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::Read;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Stacks {
    pub stacks: Vec<StackDetails>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StackDetails {
    pub name: String,
    pub description: String,
    pub stack_version: String,
    pub trunk_installs: Vec<TrunkInstall>,
    pub extensions: Vec<Extension>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrunkInstall {
    pub name: String,
    pub version: String, // needs to be parsed as a Version of semver
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extension {
    pub name: String,
    pub locations: Vec<ExtensionLocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionLocation {
    pub database: String,
    pub enabled: String,
    pub version: String,
}

#[derive(Debug)]
pub struct StackError {
    pub details: String,
}

impl StackError {
    pub fn new(msg: &str) -> StackError {
        StackError {
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for StackError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for StackError {
    fn description(&self) -> &str {
        &self.details
    }
}

pub fn define_stack(args: &ArgMatches) -> Result<String, Box<dyn Error>> {
    let stacks: Stacks = define_stacks();
    let names: Vec<String> = stacks
        .stacks
        .clone()
        .into_iter()
        .map(|stack| stack.name.to_lowercase())
        .collect();

    if let Some(stack) = args.get_one::<String>("stack") {
        let given_stack = stack.to_lowercase();

        if !names.contains(&given_stack) {
            return Err(Box::new(StackError::new("- Given Stack type not valid")));
        }

        Ok(given_stack)
    } else {
        // when no stack type is provided
        Ok("standard".to_owned())
    }
}

pub fn define_stacks() -> Stacks {
    let file = "./tembo/stacks.yaml"; // TODO: move to a constant
    let mut file = File::open(file).expect("Unable to open stack config file");
    let mut contents = String::new();

    file.read_to_string(&mut contents)
        .expect("Unable to read stack config file");

    // TODO: harden, don't use unerap in production
    let stacks: Stacks = serde_yaml::from_str(&contents).unwrap();

    stacks
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, ArgAction, Command};

    #[test]
    fn define_stack_test() {
        // given a stack name that matches
        let app = Command::new("myapp").arg(
            Arg::new("stack")
                .value_parser(clap::value_parser!(String))
                .action(ArgAction::Set)
                .required(false),
        );

        let matches = app.get_matches_from(vec!["myapp", "standard"]);

        assert_eq!(define_stack(&matches).unwrap(), "standard");

        // given a stack name that does not match
        let app = Command::new("myapp").arg(
            Arg::new("stack")
                .value_parser(clap::value_parser!(String))
                .action(ArgAction::Set)
                .required(false),
        );

        let matches = app.get_matches_from(vec!["myapp", "unknown"]);

        let expected = Box::new(StackError::new("- Given Stack type not valid")).details;
        let result = define_stack(&matches).err().unwrap().to_string();
        assert_eq!(expected, result);
    }
}
