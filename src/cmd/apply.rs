use clap::{ArgMatches, Command};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    error::Error,
    fs::{self},
};
use toml::Value;

use crate::cli::{docker::Docker, file_utils::FileUtils};
use tera::Tera;

const DOCKERFILE_NAME: &str = "Dockerfile";
const POSTGRESCONF_NAME: &str = "postgres.conf";

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TemboConfig {
    pub version: String,
    pub defaults: InstanceSettings,
}

// Config struct holds to data from the `[config]` section.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct InstanceSettings {
    pub cpu: String,
    pub memory: String,
    pub storage: String,
    pub replicas: u32,
    pub postgres_configurations: HashMap<String, Value>,
    pub extensions: HashMap<String, Extension>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct PostgresConfig {
    // TODO: Change this to a generic type
    pub statement_timeout: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Extension {
    pub enabled: bool,
    pub trunk_project: Option<String>,
    pub trunk_project_version: Option<String>,
}

// Create init subcommand arguments
pub fn make_subcommand() -> Command {
    Command::new("apply").about("Applies changes to the context set using the tembo config file")
}

pub fn execute(_args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    Docker::installed_and_running()?;

    let instance_settings: HashMap<String, InstanceSettings> = get_instance_settings()?;

    let rendered_dockerfile: String = get_rendered_dockerfile(instance_settings.clone())?;

    FileUtils::create_file(
        DOCKERFILE_NAME.to_string(),
        DOCKERFILE_NAME.to_string(),
        rendered_dockerfile,
        true,
    )?;

    let rendered_migrations: String = get_rendered_migrations_file(instance_settings.clone())?;

    FileUtils::create_file(
        "extensions".to_string(),
        "migrations/1_extensions.sql".to_string(), // TODO: Improve file naming
        rendered_migrations,
        true,
    )?;

    FileUtils::create_file(
        POSTGRESCONF_NAME.to_string(),
        POSTGRESCONF_NAME.to_string(),
        get_postgres_config(instance_settings),
        true,
    )?;

    Docker::build_run()?;

    Docker::run_sqlx_migrate()?;

    Ok(())
}

pub fn get_instance_settings() -> Result<HashMap<String, InstanceSettings>, Box<dyn Error>> {
    let mut file_path = FileUtils::get_current_working_dir();
    file_path.push_str("/tembo.toml");

    let contents = match fs::read_to_string(file_path.clone()) {
        Ok(c) => c,
        Err(e) => {
            panic!("Couldn't read context file {}: {}", file_path, e);
        }
    };

    let instance_settings: HashMap<String, InstanceSettings> = match toml::from_str(&contents) {
        Ok(d) => d,
        Err(e) => {
            panic!("Unable to load data. Error: `{}`", e);
        }
    };

    Ok(instance_settings)
}

pub fn get_rendered_dockerfile(
    instance_settings: HashMap<String, InstanceSettings>,
) -> Result<String, Box<dyn Error>> {
    let filename = "Dockerfile.template";
    let filepath =
        "https://raw.githubusercontent.com/tembo-io/tembo-cli/main/tembo/Dockerfile.template";

    FileUtils::download_file(filepath, filename)?;

    let contents = match fs::read_to_string(filename) {
        Ok(c) => c,
        Err(e) => {
            panic!("Couldn't read file {}: {}", filename, e);
        }
    };

    let mut tera = Tera::new("templates/**/*").unwrap();
    let _ = tera.add_raw_template("dockerfile", &contents);
    let mut context = tera::Context::new();
    for (_key, value) in instance_settings.iter() {
        context.insert("extensions", &value.extensions);
    }
    let rendered_dockerfile = tera.render("dockerfile", &context).unwrap();

    Ok(rendered_dockerfile)
}

pub fn get_rendered_migrations_file(
    instance_settings: HashMap<String, InstanceSettings>,
) -> Result<String, Box<dyn Error>> {
    let filename = "migrations.sql.template";
    let filepath =
        "https://raw.githubusercontent.com/tembo-io/tembo-cli/main/tembo/migrations.sql.template";

    FileUtils::download_file(filepath, filename)?;

    let contents = match fs::read_to_string(filename) {
        Ok(c) => c,
        Err(e) => {
            panic!("Couldn't read file {}: {}", filename, e);
        }
    };

    let mut tera = Tera::new("templates/**/*").unwrap();
    let _ = tera.add_raw_template("migrations", &contents);
    let mut context = tera::Context::new();
    for (_key, value) in instance_settings.iter() {
        context.insert("extensions", &value.extensions);
    }
    let rendered_dockerfile = tera.render("migrations", &context).unwrap();

    Ok(rendered_dockerfile)
}

fn get_postgres_config(instance_settings: HashMap<String, InstanceSettings>) -> String {
    let mut postgres_config = String::from("");
    let qoute_new_line = "\'\n";
    let equal_to_qoute = " = \'";
    for (_, instance_setting) in instance_settings.iter() {
        for (key, value) in instance_setting.postgres_configurations.iter() {
            if value.is_str() {
                postgres_config.push_str(key.as_str());
                postgres_config.push_str(equal_to_qoute);
                postgres_config.push_str(value.as_str().unwrap());
                postgres_config.push_str(qoute_new_line);
            }
            if value.is_table() {
                for row in value.as_table().iter() {
                    for (t, v) in row.iter() {
                        postgres_config.push_str(key.as_str());
                        postgres_config.push_str(".");
                        postgres_config.push_str(t.as_str());
                        postgres_config.push_str(equal_to_qoute);
                        postgres_config.push_str(v.as_str().unwrap());
                        postgres_config.push_str(qoute_new_line);
                    }
                }
            }
        }
    }
    return postgres_config.to_string();
}
