use std::fmt::Display;
use std::fs;

use anyhow::Error;
use anyhow::{anyhow, bail};
use serde::Deserialize;
use serde::Serialize;

use crate::tui;

pub const CONTEXT_EXAMPLE_TEXT: &str = "version = \"1.0\"

[[environment]]
name = 'local'
target = 'docker'
    
[[environment]]
name = 'prod'
target = 'tembo-cloud'
org_id = 'ORG_ID'
profile = 'prod'
set = true";

pub const CREDENTIALS_EXAMPLE_TEXT: &str = "version = \"1.0\"
    
[[profile]]
name = 'prod'
tembo_access_token = 'ACCESS_TOKEN'
tembo_host = 'https://api.tembo.io'
tembo_data_host = 'https://api.data-1.use1.tembo.io'
";

pub const CONTEXT_DEFAULT_TEXT: &str = "version = \"1.0\"

[[environment]]
name = 'local'
target = 'docker'
set = true

# [[environment]]
# name = 'prod'
# target = 'tembo-cloud'
# org_id can be found in your tembo cloud url. Example: org_2bVDi36rsJNot2gwzP37enwxzMk
# org_id = 'Org ID here'
# profile = 'prod'
";

pub const CREDENTIALS_DEFAULT_TEXT: &str = "version = \"1.0\"

# Remove commented out profile to setup your environment

# [[profile]]
# name = 'prod'
# Generate an Access Token either through 'tembo login' or visit 'https://cloud.tembo.io/generate-jwt'
# tembo_access_token = 'Access token here'
# tembo_host = 'https://api.tembo.io'
# tembo_data_host = 'https://api.data-1.use1.tembo.io'
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
        self.tembo_data_host.trim_end_matches('/').to_string()
    }

    pub fn get_tembo_host(&self) -> String {
        self.tembo_host.trim_end_matches('/').to_string()
    }
}

impl Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Docker => f.write_str("docker"),
            Target::TemboCloud => f.write_str("tembo-cloud"),
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
        Ok(mut context) => {
            let mut count = 0;
            for e in context.environment.iter_mut() {
                if e.set == Some(true) {
                    count += 1;
                }
            }

            if count > 1 {
                bail!("More than one environment is set to true. Check your context file");
            } else {
                Ok(Some(context))
            }
        }
        Err(err) => {
            eprintln!("\nInvalid context file {filename}\n");

            tui::error(&format!("Error: {}", err.message()));

            eprintln!("\nExample context file: \n\n{}", CONTEXT_EXAMPLE_TEXT);

            Err(Error::msg("Error listing tembo context!"))
        }
    }
}

pub fn get_current_context() -> Result<Environment, anyhow::Error> {
    let maybe_context = list_context()?;

    if let Some(context) = maybe_context {
        for env in context.environment {
            if let Some(true) = env.set {
                if env.name == "local" {
                    return Ok(env);
                } else {
                    let maybe_profiles = list_credential_profiles()?;

                    if let Some(profiles) = maybe_profiles {
                        if let Some(profile_name) = &env.profile {
                            let credential = profiles
                                .iter()
                                .rev()
                                .find(|c| &c.name == profile_name)
                                .ok_or_else(|| anyhow!("Profile not found in credentials"))?;

                            let mut env_with_profile = env.clone();
                            env_with_profile.selected_profile = Some(credential.clone());

                            return Ok(env_with_profile);
                        } else {
                            bail!("Environment is not set up properly. Check out your context");
                        }
                    } else {
                        bail!("Credentials file not found or invalid");
                    }
                }
            }
        }
    }

    bail!("Environment is not set up properly. Check out your context");
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
                CREDENTIALS_EXAMPLE_TEXT
            );

            Err(Error::msg("Error listing tembo credentials profiles!"))
        }
    }
}
