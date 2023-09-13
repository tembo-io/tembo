// Stacks are defined templates provided by Tembo containing attributes and extensions
// (templates contain configuration information tailored to a specific use case)

use crate::cli::stack_error::StackError;
use crate::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use clap::ArgMatches;
use std::error::Error;
use std::fs::File;
use std::io::Read;

// object containing all of the defined stacks
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Stacks {
    pub stacks: Vec<Stack>,
}

// TODO: give a stack a unique id (name + version)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    pub name: String,
    pub description: String,
    pub version: String,
    pub trunk_installs: Vec<TrunkInstall>,
    pub extensions: Vec<Extension>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrunkInstall {
    pub name: Option<String>,
    pub version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extension {
    pub name: Option<String>,
    pub version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub locations: Vec<ExtensionLocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionLocation {
    pub database: String,
    pub enabled: String,
    pub version: String,
}

// returns a result containing the stack name
pub fn define_stack(args: &ArgMatches) -> Result<String, Box<dyn Error>> {
    let stacks: Stacks = define_stacks();
    let names: Vec<String> = stacks
        .stacks
        .clone()
        .into_iter()
        .map(|stack| stack.name.to_lowercase())
        .collect();

    let passed = args.try_get_one::<String>("stack");

    if let Ok(Some(stack_option)) = passed {
        let given_stack = stack_option.to_lowercase();

        if !names.contains(&given_stack) {
            return Err(Box::new(StackError::new("- Given Stack type not valid")));
        }

        Ok(given_stack)
    } else {
        // when no stack type is provided
        Ok("standard".to_owned())
    }
}

// returns a Stacks object containing attributes loaded from the stacks.yml file
pub fn define_stacks() -> Stacks {
    let file = "./tembo/stacks.yaml"; // TODO: move to a constant
    let mut file = File::open(file).expect("Unable to open stack config file");
    let mut contents = String::new();

    file.read_to_string(&mut contents)
        .expect("Unable to read stack config file");

    serde_yaml::from_str(&contents).unwrap()
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
