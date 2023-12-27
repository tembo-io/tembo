use anyhow::Error;
use clap::Args;
use controller::stacks::get_stack;
use controller::stacks::types::StackType as ControllerStackType;
use log::info;
use spinners::{Spinner, Spinners};
use std::{
    collections::HashMap,
    fs::{self},
    str::FromStr,
    thread::sleep,
    time::Duration,
};
use temboclient::{
    apis::{
        configuration::Configuration,
        instance_api::{create_instance, get_all, get_instance, put_instance},
    },
    models::{
        ConnectionInfo, Cpu, CreateInstance, Extension, ExtensionInstallLocation, Memory, PgConfig,
        StackType, State, Storage, TrunkInstall, UpdateInstance,
    },
};
use tembodataclient::apis::secrets_api::get_secret_v1;
use tokio::runtime::Runtime;

use crate::cli::context::{get_current_context, Environment, Profile, Target};
use crate::cli::docker::Docker;
use crate::cli::file_utils::FileUtils;
use crate::cli::sqlx_utils::SqlxUtils;
use crate::cli::tembo_config;
use crate::cli::tembo_config::InstanceSettings;
use tera::Tera;

const DOCKERFILE_NAME: &str = "Dockerfile";
const POSTGRESCONF_NAME: &str = "postgres.conf";

/// Deploys a tembo.toml file
#[derive(Args)]
pub struct ApplyCommand {}

pub fn execute() -> Result<(), anyhow::Error> {
    info!("Running validation!");
    super::validate::execute()?;
    info!("Validation completed!");

    let env = get_current_context()?;

    if env.target == Target::Docker.to_string() {
        return execute_docker();
    } else if env.target == Target::TemboCloud.to_string() {
        return execute_tembo_cloud(env.clone());
    }

    Ok(())
}

fn execute_docker() -> Result<(), anyhow::Error> {
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
        get_postgres_config(instance_settings.clone()),
        true,
    )?;

    for (_key, value) in instance_settings.iter() {
        let port = Docker::build_run(value.instance_name.clone())?;

        // Allows DB instance to be ready before running migrations
        sleep(Duration::from_secs(3));

        let conn_info = ConnectionInfo {
            host: "localhost".to_owned(),
            pooler_host: Some(Some("localhost-pooler".to_string())),
            port,
            user: "postgres".to_owned(),
            password: "postgres".to_owned(),
        };
        Runtime::new()
            .unwrap()
            .block_on(SqlxUtils::run_migrations(conn_info))?;

        // If all of the above was successful, we can print the url to user
        println!(
            ">>> Tembo instance is now running on: postgres://postgres:postgres@localhost:{}",
            port
        );
    }

    Ok(())
}

pub fn execute_tembo_cloud(env: Environment) -> Result<(), anyhow::Error> {
    let instance_settings: HashMap<String, InstanceSettings> = get_instance_settings()?;

    let profile = env.clone().selected_profile.unwrap();
    let config = Configuration {
        base_path: profile.clone().tembo_host,
        bearer_access_token: Some(profile.clone().tembo_access_token),
        ..Default::default()
    };

    for (_key, value) in instance_settings.iter() {
        let mut instance_id = get_instance_id(value.instance_name.clone(), &config, env.clone())?;

        if let Some(env_instance_id) = instance_id.clone() {
            update_existing_instance(env_instance_id, value, &config, env.clone());
        } else {
            instance_id = create_new_instance(value, &config, env.clone());
        }

        loop {
            let mut sp = Spinner::new(Spinners::Line, "Waiting for instance to be up!".into());
            sleep(Duration::from_secs(10));

            let connection_info: Option<Box<ConnectionInfo>> =
                is_instance_up(instance_id.as_ref().unwrap().clone(), &config, &env)?;

            if connection_info.is_some() {
                let conn_info = get_conn_info_with_creds(
                    profile.clone(),
                    &instance_id,
                    connection_info,
                    env.clone(),
                )?;

                Runtime::new()
                    .unwrap()
                    .block_on(SqlxUtils::run_migrations(conn_info))?;

                sp.stop_with_message("- Instance is now up!".to_string());

                break;
            }
        }
    }

    Ok(())
}

fn get_conn_info_with_creds(
    profile: Profile,
    instance_id: &Option<String>,
    connection_info: Option<Box<ConnectionInfo>>,
    env: Environment,
) -> Result<ConnectionInfo, anyhow::Error> {
    let dataplane_config = tembodataclient::apis::configuration::Configuration {
        base_path: profile.tembo_data_host,
        bearer_access_token: Some(profile.tembo_access_token),
        ..Default::default()
    };

    let result = Runtime::new().unwrap().block_on(get_secret_v1(
        &dataplane_config,
        env.org_id.clone().unwrap().as_str(),
        instance_id.as_ref().unwrap(),
        "superuser-role",
    ));

    if result.is_err() {
        return Err(Error::msg("Error fetching instance credentials!"));
    }

    let mut conn_info = *connection_info.unwrap();

    let map = result.as_ref().unwrap();

    conn_info.user = map.get("username").unwrap().to_string();
    conn_info.password = map.get("password").unwrap().to_string();

    Ok(conn_info)
}

pub fn get_instance_id(
    instance_name: String,
    config: &Configuration,
    env: Environment,
) -> Result<Option<String>, anyhow::Error> {
    let v = Runtime::new()
        .unwrap()
        .block_on(get_all(config, env.org_id.clone().unwrap().as_str()));

    match v {
        Ok(result) => {
            let maybe_instance = result
                .iter()
                .find(|instance| instance.instance_name == instance_name);

            if let Some(instance) = maybe_instance {
                return Ok(Some(instance.clone().instance_id));
            }
        }
        Err(error) => eprintln!("Error getting instance: {}", error),
    };
    Ok(None)
}

pub fn is_instance_up(
    instance_id: String,
    config: &Configuration,
    env: &Environment,
) -> Result<Option<Box<ConnectionInfo>>, anyhow::Error> {
    let v = Runtime::new().unwrap().block_on(get_instance(
        config,
        env.org_id.clone().unwrap().as_str(),
        &instance_id,
    ));

    match v {
        Ok(result) => {
            if result.state == State::Up {
                return Ok(result.connection_info.unwrap());
            }
        }
        Err(error) => {
            eprintln!("Error getting instance: {}", error);
            return Err(Error::new(error));
        }
    };

    Ok(None)
}

fn update_existing_instance(
    instance_id: String,
    value: &InstanceSettings,
    config: &Configuration,
    env: Environment,
) {
    let instance = get_update_instance(value);

    let v = Runtime::new().unwrap().block_on(put_instance(
        config,
        env.org_id.clone().unwrap().as_str(),
        &instance_id,
        instance,
    ));

    match v {
        Ok(result) => {
            println!(
                "Instance update started for Instance Id: {}",
                result.instance_id
            );
        }
        Err(error) => eprintln!("Error updating instance: {}", error),
    };
}

fn create_new_instance(
    value: &InstanceSettings,
    config: &Configuration,
    env: Environment,
) -> Option<String> {
    let instance = get_create_instance(value);

    let v = Runtime::new().unwrap().block_on(create_instance(
        config,
        env.org_id.clone().unwrap().as_str(),
        instance,
    ));

    match v {
        Ok(result) => {
            println!(
                "Instance creation started for instance_name: {} with instance_id: {}",
                result.instance_name, result.instance_id
            );

            return Some(result.instance_id);
        }
        Err(error) => {
            eprintln!("Error creating instance: {}", error);
        }
    };

    None
}

fn get_create_instance(instance_settings: &InstanceSettings) -> CreateInstance {
    return CreateInstance {
        cpu: Cpu::from_str(instance_settings.cpu.as_str()).unwrap(),
        memory: Memory::from_str(instance_settings.memory.as_str()).unwrap(),
        environment: temboclient::models::Environment::from_str(
            instance_settings.environment.as_str(),
        )
        .unwrap(),
        instance_name: instance_settings.instance_name.clone(),
        stack_type: StackType::from_str(instance_settings.stack_type.as_str()).unwrap(),
        storage: Storage::from_str(instance_settings.storage.as_str()).unwrap(),
        replicas: Some(instance_settings.replicas),
        app_services: None,
        connection_pooler: None,
        extensions: Some(Some(get_extensions(instance_settings.extensions.clone()))),
        extra_domains_rw: None,
        ip_allow_list: None,
        trunk_installs: Some(Some(get_trunk_installs(
            instance_settings.extensions.clone(),
        ))),
        postgres_configs: Some(Some(get_postgres_config_cloud(instance_settings))),
    };
}

fn get_update_instance(instance_settings: &InstanceSettings) -> UpdateInstance {
    return UpdateInstance {
        cpu: Cpu::from_str(instance_settings.cpu.as_str()).unwrap(),
        memory: Memory::from_str(instance_settings.memory.as_str()).unwrap(),
        environment: temboclient::models::Environment::from_str(
            instance_settings.environment.as_str(),
        )
        .unwrap(),
        storage: Storage::from_str(instance_settings.storage.as_str()).unwrap(),
        replicas: instance_settings.replicas,
        app_services: None,
        connection_pooler: None,
        extensions: Some(Some(get_extensions(instance_settings.extensions.clone()))),
        extra_domains_rw: None,
        ip_allow_list: None,
        trunk_installs: Some(Some(get_trunk_installs(
            instance_settings.extensions.clone(),
        ))),
        postgres_configs: Some(Some(get_postgres_config_cloud(instance_settings))),
    };
}

fn get_postgres_config_cloud(instance_settings: &InstanceSettings) -> Vec<PgConfig> {
    let mut pg_configs: Vec<PgConfig> = vec![];

    if instance_settings.postgres_configurations.is_some() {
        for (key, value) in instance_settings
            .postgres_configurations
            .clone()
            .unwrap()
            .iter()
        {
            if value.is_str() {
                pg_configs.push(PgConfig {
                    name: key.to_owned(),
                    value: value.to_string(),
                })
            } else if value.is_table() {
                for row in value.as_table().iter() {
                    for (k, v) in row.iter() {
                        pg_configs.push(PgConfig {
                            name: key.to_owned() + "." + k,
                            value: v.to_string(),
                        })
                    }
                }
            }
        }
    }

    pg_configs
}

fn get_extensions(extensions: Option<HashMap<String, tembo_config::Extension>>) -> Vec<Extension> {
    let mut vec_extensions: Vec<Extension> = vec![];
    let mut vec_extension_location: Vec<ExtensionInstallLocation> = vec![];

    if extensions.is_some() {
        for (name, extension) in extensions.unwrap().iter() {
            vec_extension_location.push(ExtensionInstallLocation {
                database: None,
                schema: None,
                version: None,
                enabled: extension.enabled,
            });

            vec_extensions.push(Extension {
                name: name.to_owned(),
                description: None,
                locations: vec_extension_location.clone(),
            });
        }
    }

    vec_extensions
}

fn get_trunk_installs(
    extensions: Option<HashMap<String, tembo_config::Extension>>,
) -> Vec<TrunkInstall> {
    let mut vec_trunk_installs: Vec<TrunkInstall> = vec![];

    if extensions.is_some() {
        for (_, extension) in extensions.unwrap().iter() {
            if extension.trunk_project.is_some() {
                vec_trunk_installs.push(TrunkInstall {
                    name: extension.trunk_project.clone().unwrap(),
                    version: Some(extension.trunk_project_version.clone()),
                });
            }
        }
    }
    vec_trunk_installs
}

pub fn get_instance_settings() -> Result<HashMap<String, InstanceSettings>, anyhow::Error> {
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
) -> Result<String, anyhow::Error> {
    let filename = "Dockerfile.template";
    let filepath =
        "https://raw.githubusercontent.com/tembo-io/tembo/main/tembo-cli/tembo/Dockerfile.template";

    FileUtils::download_file(filepath, filename, true)?;

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
        let stack_type = ControllerStackType::from_str(value.stack_type.as_str())
            .unwrap_or(ControllerStackType::Standard);

        let stack = get_stack(stack_type);

        context.insert("stack_trunk_installs", &stack.trunk_installs);
        context.insert("extensions", &value.extensions);
    }
    let rendered_dockerfile = tera.render("dockerfile", &context).unwrap();

    Ok(rendered_dockerfile)
}

pub fn get_rendered_migrations_file(
    instance_settings: HashMap<String, InstanceSettings>,
) -> Result<String, anyhow::Error> {
    let filename = "migrations.sql.template";
    let filepath =
        "https://raw.githubusercontent.com/tembo-io/tembo/main/tembo-cli/tembo/migrations.sql.template";

    FileUtils::download_file(filepath, filename, true)?;

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
        let stack_type = ControllerStackType::from_str(value.stack_type.as_str())
            .unwrap_or(ControllerStackType::Standard);

        let stack = get_stack(stack_type);

        context.insert("stack_extensions", &stack.extensions);
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
        let stack_type = ControllerStackType::from_str(instance_setting.stack_type.as_str())
            .unwrap_or(ControllerStackType::Standard);

        let stack = get_stack(stack_type);

        if stack.postgres_config.is_some() {
            for config in stack.postgres_config.unwrap().iter() {
                postgres_config.push_str(config.name.as_str());
                postgres_config.push_str(equal_to_qoute);
                postgres_config.push_str(format!("{}", &config.value).as_str());
                postgres_config.push_str(qoute_new_line);
            }
        }

        if instance_setting.postgres_configurations.is_some() {
            for (key, value) in instance_setting
                .postgres_configurations
                .as_ref()
                .unwrap()
                .iter()
            {
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
                            postgres_config.push('.');
                            postgres_config.push_str(t.as_str());
                            postgres_config.push_str(equal_to_qoute);
                            postgres_config.push_str(v.as_str().unwrap());
                            postgres_config.push_str(qoute_new_line);
                        }
                    }
                }
            }
        }
    }

    postgres_config
}
