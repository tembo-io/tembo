use std::fs;

use crate::Result;
use anyhow::bail;
use anyhow::Ok;
use serde::Deserialize;
use serde::Serialize;

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
}

pub enum Target {
    Docker,
    TemboCloud,
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

pub fn list_context() -> Result<Context> {
    let filename = tembo_context_file_path();

    let contents = match fs::read_to_string(filename.clone()) {
        std::result::Result::Ok(c) => c,
        Err(e) => {
            bail!("Error reading file {filename}: {e}")
        }
    };

    let context: Context = match toml::from_str(&contents) {
        std::result::Result::Ok(c) => c,
        Err(e) => {
            bail!("Issue with format of toml file {filename}: {e}")
        }
    };

    Ok(context)
}

pub fn get_current_context() -> Result<Environment> {
    let context = list_context()?;

    let profiles = list_credentail_profiles()?;

    for mut e in context.environment {
        if e.set.is_some() && e.set.unwrap() {
            if e.profile.is_some() {
                let credential = profiles
                    .iter()
                    .filter(|c| &c.name == e.profile.as_ref().unwrap())
                    .last()
                    .unwrap();

                e.selected_profile = Some(credential.to_owned());
            }
            return Ok(e);
        }
    }

    bail!("Tembo context not set");
}

pub fn list_credentail_profiles() -> Result<Vec<Profile>> {
    let filename = tembo_credentials_file_path();

    let contents = match fs::read_to_string(filename.clone()) {
        std::result::Result::Ok(c) => c,
        Err(e) => {
            bail!("Error reading file {filename}: {e}")
        }
    };

    let credential: Credential = match toml::from_str(&contents) {
        std::result::Result::Ok(c) => c,
        Err(e) => {
            bail!("Issue with format of toml file {filename}: {e}")
        }
    };

    Ok(credential.profile)
}
