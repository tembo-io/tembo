use crate::{
    cli::{
        context::{get_current_context, tembo_state_file_path, Environment, Target},
        tembo_config,
    },
    Result,
};
use clap::{ArgMatches, Command};
use controller::stacks::get_stack;
use controller::stacks::types::StackType as ControllerStackType;
use std::io::Write;
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    str::FromStr,
};
use temboclient::{
    apis::{
        configuration::Configuration,
        instance_api::{create_instance, put_instance},
    },
    models::{
        Cpu, CreateInstance, Extension, ExtensionInstallLocation, Memory, PgConfig, StackType,
        Storage, TrunkInstall, UpdateInstance,
    },
};
use tokio::runtime::Runtime;

use crate::cli::{docker::Docker, file_utils::FileUtils, tembo_config::InstanceSettings};
use tera::Tera;

const DOCKERFILE_NAME: &str = "Dockerfile";
const POSTGRESCONF_NAME: &str = "postgres.conf";

// Create init subcommand arguments
pub fn make_subcommand() -> Command {
    Command::new("apply").about("Applies changes to the context set using the tembo config file")
}

pub fn execute(_args: &ArgMatches) -> Result<()> {
    let env = get_current_context()?;

    if env.target == Target::Docker.to_string() {
        return execute_docker();
    } else if env.target == Target::TemboCloud.to_string() {
        return execute_tembo_cloud(env);
    }

    Ok(())
}

fn execute_docker() -> Result<()> {
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

    // If all of the above was successful, we can print the url to user
    println!(">>> Tembo instance is now running on: postgres://postgres:postgres@localhost:5432");

    Ok(())
}

pub fn execute_tembo_cloud(env: Environment) -> Result<()> {
    let instance_settings: HashMap<String, InstanceSettings> = get_instance_settings()?;

    let profile = env.clone().selected_profile.unwrap();
    let config = Configuration {
        base_path: profile.tembo_host,
        bearer_access_token: Some(profile.tembo_access_token),
        ..Default::default()
    };

    for (_key, value) in instance_settings.iter() {
        let instance_id = get_instance_id_from_state(value.instance_name.clone())?;
        if let Some(env_instance_id) = instance_id {
            update_existing_instance(env_instance_id, value, &config, env.clone());
        } else {
            create_new_instance(value, &config, env.clone());
        }
    }

    Ok(())
}

pub fn get_instance_id_from_state(instance_name: String) -> Result<Option<String>> {
    let contents = fs::read_to_string(tembo_state_file_path())?;

    let tembo_state_map: HashMap<String, String> = toml::from_str(&contents)?;

    let tembo_state = tembo_state_map.get(&instance_name);
    if tembo_state.is_none() {
        Ok(None)
    } else {
        let instance_id = tembo_state.unwrap().clone();
        Ok(Some(instance_id))
    }
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
            )
        }
        Err(error) => eprintln!("Error updating instance: {}", error),
    };
}

fn create_new_instance(value: &InstanceSettings, config: &Configuration, env: Environment) {
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

            let mut state_file = OpenOptions::new()
                .append(true)
                .open(tembo_state_file_path())
                .expect("cannot open file");

            let state = format!("{} = \"{}\"\n", result.instance_name, result.instance_id);

            state_file
                .write_all(state.as_bytes())
                .expect("write failed");
        }
        Err(error) => eprintln!("Error creating instance: {}", error),
    };
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

pub fn get_instance_settings() -> Result<HashMap<String, InstanceSettings>> {
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
) -> Result<String> {
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
) -> Result<String> {
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
