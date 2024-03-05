use std::fs;

use anyhow::Error;
use anyhow::{anyhow, bail};
use serde::Deserialize;
use serde::Serialize;

use crate::tui;

// TODO: Move this to a template file
pub const CONTEXT_DEFAULT_TEXT: &str = "version = \"1.0\"

[[environment]]
name = 'local'
target = 'docker'
set = true

[[environment]]
name = 'prod'
target = 'tembo-cloud'
org_id = 'ORG_ID'
profile = 'prod'
";

// TODO: Move this to a template file
pub const CREDENTIALS_DEFAULT_TEXT: &str = "version = \"1.0\"

[[profile]]
name = 'prod'
tembo_access_token = 'ACCESS_TOKEN'
tembo_host = 'https://api.tembo.io'
tembo_data_host = 'https://api.data-1.use1.tembo.io'
";

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Context {
    pub version: String,
    pub environment: Vec<Environment>,
}

// Config struct holds to data from the `[config]` section.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Environment {
    pub name: String,
    pub target: String,
    pub org_id: Option<String>,
    pub profile: Option<String>,
    pub set: Option<bool>,
    pub selected_profile: Option<Profile>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Credential {
    pub version: String,
    pub profile: Vec<Profile>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Profile {
    pub name: String,
    pub tembo_access_token: String,
    pub tembo_host: String,
    pub tembo_data_host: String,
}

pub enum Target {
    Docker,
    TemboCloud,
}

impl Profile {
    pub fn get_tembo_data_host(&self) -> String {
        return self.tembo_data_host.trim_end_matches('/').to_string();
    }

    pub fn get_tembo_host(&self) -> String {
        return self.tembo_host.trim_end_matches('/').to_string();
    }
}

impl ToString for Target {
    fn to_string(&self) -> String {
        match self {
            Self::Docker => String::from("docker"),
            Self::TemboCloud => String::from("tembo-cloud"),
        }
    }
}

pub fn tembo_home_dir() -> String {
    let mut tembo_home = home::home_dir().unwrap().as_path().display().to_string();
    tembo_home.push_str("/.tembo");
    tembo_home
}

pub fn tembo_context_file_path() -> String {
    tembo_home_dir() + "/context"
}

pub fn tembo_credentials_file_path() -> String {
    tembo_home_dir() + "/credentials"
}

pub fn list_context() -> Result<Option<Context>, anyhow::Error> {
    let filename = tembo_context_file_path();

    let contents = fs::read_to_string(&filename)
        .map_err(|err| anyhow!("Error reading file {filename}: {err}"))?;

    let maybe_context: Result<Context, toml::de::Error> = toml::from_str(&contents);

    match maybe_context {
        Ok(context) => Ok(Some(context)),
        Err(err) => {
            eprintln!("\nInvalid context file {filename}\n");

            tui::error(&format!("Error: {}", err.message()));

            eprintln!("\nExample context file: \n\n{}", CONTEXT_DEFAULT_TEXT);

            Err(Error::msg("Error listing tembo context!"))
        }
    }
}

pub fn get_current_context() -> Result<Environment, anyhow::Error> {
    let maybe_context = list_context()?;

    if let Some(context) = maybe_context {
        let maybe_profiles = list_credential_profiles()?;

        if let Some(profiles) = maybe_profiles {
            for mut env in context.environment {
                if let Some(_is_set) = env.set {
                    if let Some(profile) = &env.profile {
                        let credential =
                            profiles.iter().rev().find(|c| &c.name == profile).unwrap();

                        env.selected_profile = Some(credential.to_owned());
                    }

                    return Ok(env);
                }
            }
        }
    }

    bail!("Tembo context not set");
}

pub fn list_credential_profiles() -> Result<Option<Vec<Profile>>, anyhow::Error> {
    let filename = tembo_credentials_file_path();

    let contents = fs::read_to_string(&filename)
        .map_err(|err| anyhow!("Error reading file {filename}: {err}"))?;

    let maybe_credential: Result<Credential, toml::de::Error> = toml::from_str(&contents);

    match maybe_credential {
        Ok(credential) => Ok(Some(credential.profile)),
        Err(err) => {
            eprintln!("\nInvalid credentials file {filename}\n");

            tui::error(&format!("Error: {}", err.message()));

            eprintln!(
                "\nExample credentials file: \n\n{}",
                CREDENTIALS_DEFAULT_TEXT
            );

            Err(Error::msg("Error listing tembo credentials profiles!"))
        }
    }
}
