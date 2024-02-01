use anyhow::Context as AnyhowContext;
use anyhow::Error;
use clap::Args;
use colorful::Colorful;
use controller::stacks::get_stack;
use controller::stacks::types::StackType as ControllerStackType;
use log::info;
use std::path::Path;
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

use crate::cli::docker::Docker;
use crate::cli::file_utils::FileUtils;
use crate::cli::sqlx_utils::SqlxUtils;
use crate::cli::tembo_config;
use crate::cli::tembo_config::InstanceSettings;
use crate::cli::tembo_config::OverlayInstanceSettings;
use crate::tui;
use crate::{
    cli::context::{get_current_context, Environment, Profile, Target},
    tui::{clean_console, colors, instance_started, white_confirmation},
};
use tera::{Context, Tera};

const DOCKERFILE_NAME: &str = "Dockerfile";
const DOCKERCOMPOSE_NAME: &str = "docker-compose.yml";
const POSTGRESCONF_NAME: &str = "postgres.conf";

/// Deploys a tembo.toml file
#[derive(Args)]
pub struct ApplyCommand {
    #[clap(long, short = 'm')]
    pub merge: Option<String>,
    #[clap(long, short = 's')]
    pub set: Option<String>,
}

pub fn execute(
    verbose: bool,
    merge_path: Option<String>,
    set_arg: Option<String>,
) -> Result<(), anyhow::Error> {
    info!("Running validation!");
    super::validate::execute(verbose)?;
    info!("Validation completed!");

    let env = get_current_context()?;

    let instance_settings = get_instance_settings(merge_path, set_arg)?;

    if env.target == Target::Docker.to_string() {
        return docker_apply(verbose, instance_settings);
    } else if env.target == Target::TemboCloud.to_string() {
        return tembo_cloud_apply(env, instance_settings);
    }

    Ok(())
}

fn parse_set_arg(set_arg: &str) -> Result<(String, String, String), Error> {
    let parts: Vec<&str> = set_arg.split('=').collect();
    if parts.len() != 2 {
        println!("Error: Invalid format (missing '=')");
        return Err(Error::msg("Invalid format for --set"));
    }

    let key_parts: Vec<&str> = parts[0].split('.').collect();
    if key_parts.len() != 2 {
        println!("Error: Invalid format (missing '.')");
        return Err(Error::msg("Invalid format for --set"));
    }

    let instance_name = key_parts[0].to_string();
    let setting_name = key_parts[1].to_string();
    let setting_value = parts[1].to_string();

    Ok((instance_name, setting_name, setting_value))
}

fn tembo_cloud_apply(
    env: Environment,
    instance_settings: HashMap<String, InstanceSettings>,
) -> Result<(), anyhow::Error> {
    for (_key, instance_setting) in instance_settings.iter() {
        let result = tembo_cloud_apply_instance(&env, instance_setting);

        match result {
            Ok(i) => i,
            Err(error) => {
                tui::error(&format!("Error creating instance: {}", error));
                return Ok(());
            }
        }
    }

    Ok(())
}

fn docker_apply(
    verbose: bool,
    instance_settings: HashMap<String, InstanceSettings>,
) -> Result<(), anyhow::Error> {
    Docker::installed_and_running()?;

    Docker::docker_compose_down(false)?;

    for (_key, instance_setting) in instance_settings.clone().iter() {
        let result = docker_apply_instance(verbose, instance_setting);

        match result {
            Ok(i) => i,
            Err(error) => {
                tui::error(&format!("Error creating instance: {}", error));
                return Ok(());
            }
        }
    }

    let rendered_dockercompose: String = get_rendered_dockercompose(instance_settings.clone())?;

    FileUtils::create_file(
        DOCKERCOMPOSE_NAME.to_string(),
        DOCKERCOMPOSE_NAME.to_string(),
        rendered_dockercompose,
        true,
    )?;

    Docker::docker_compose_up(verbose)?;

    // Allows DB instance to be ready before running migrations
    sleep(Duration::from_secs(5));

    let port = 5432;

    for (_key, instance_setting) in instance_settings.clone().iter() {
        let conn_info = ConnectionInfo {
            host: format!("{}.local.tembo.io", instance_setting.instance_name).to_owned(),
            pooler_host: Some(Some("localhost-pooler".to_string())),
            port,
            user: "postgres".to_owned(),
            password: "postgres".to_owned(),
        };
        Runtime::new().unwrap().block_on(SqlxUtils::run_migrations(
            conn_info,
            instance_setting.instance_name.clone(),
        ))?;

        // If all of the above was successful, we can print the url to user
        instance_started(
            &format!(
                "postgres://postgres:postgres@{}.local.tembo.io:{}",
                instance_setting.instance_name, port
            ),
            &instance_setting.stack_type,
            "local",
        );
    }

    Ok(())
}

fn docker_apply_instance(
    verbose: bool,
    instance_settings: &InstanceSettings,
) -> Result<(), anyhow::Error> {
    FileUtils::create_dir(
        instance_settings.instance_name.clone(),
        instance_settings.instance_name.clone(),
    )?;

    let rendered_dockerfile: String = get_rendered_dockerfile(instance_settings)?;

    FileUtils::create_file(
        DOCKERFILE_NAME.to_string(),
        instance_settings.instance_name.clone() + "/" + DOCKERFILE_NAME,
        rendered_dockerfile,
        true,
    )?;

    let rendered_migrations: String = get_rendered_migrations_file(instance_settings)?;

    FileUtils::create_file(
        "extensions".to_string(),
        instance_settings.instance_name.clone() + "/" + "migrations/1_extensions.sql",
        rendered_migrations,
        true,
    )?;

    FileUtils::create_file(
        POSTGRESCONF_NAME.to_string(),
        instance_settings.instance_name.clone() + "/" + POSTGRESCONF_NAME,
        get_postgres_config(instance_settings),
        true,
    )?;

    Docker::build(instance_settings.instance_name.clone(), verbose)?;

    Ok(())
}

pub fn tembo_cloud_apply_instance(
    env: &Environment,
    instance_settings: &InstanceSettings,
) -> Result<(), anyhow::Error> {
    let profile = env
        .selected_profile
        .as_ref()
        .with_context(|| "Expected [environment] to have a selected profile")?;
    let config = Configuration {
        base_path: profile.tembo_host.clone(),
        bearer_access_token: Some(profile.tembo_access_token.clone()),
        ..Default::default()
    };

    let mut instance_id = get_instance_id(&instance_settings.instance_name, &config, env)?;

    if let Some(env_instance_id) = &instance_id {
        update_existing_instance(env_instance_id, instance_settings, &config, env);
    } else {
        let new_inst_req = create_new_instance(instance_settings, &config, env.clone());
        match new_inst_req {
            Ok(new_instance_id) => instance_id = Some(new_instance_id),
            Err(error) => {
                tui::error(&format!("Error creating instance: {}", error));
                return Ok(());
            }
        }
    }
    println!();
    let mut sp = spinoff::Spinner::new(
        spinoff::spinners::Aesthetic,
        "Waiting for instance to be up...",
        colors::SPINNER_COLOR,
    );
    loop {
        sleep(Duration::from_secs(5));

        let connection_info: Option<Box<ConnectionInfo>> =
            is_instance_up(instance_id.as_ref().unwrap().clone(), &config, env)?;

        if connection_info.is_some() {
            let conn_info = get_conn_info_with_creds(
                profile.clone(),
                &instance_id,
                connection_info,
                env.clone(),
            )?;

            if Path::new(&instance_settings.instance_name).exists() {
                Runtime::new().unwrap().block_on(SqlxUtils::run_migrations(
                    conn_info.clone(),
                    instance_settings.instance_name.clone(),
                ))?;
            }

            // If all of the above was successful we can stop the spinner and show a success message
            sp.stop_with_message(&format!(
                "{} {}",
                "✓".color(colors::indicator_good()).bold(),
                "Instance is up!".bold()
            ));
            clean_console();
            let connection_string = construct_connection_string(conn_info);
            instance_started(&connection_string, &instance_settings.stack_type, "cloud");

            break;
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
    instance_name: &str,
    config: &Configuration,
    env: &Environment,
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
    instance_id: &str,
    value: &InstanceSettings,
    config: &Configuration,
    env: &Environment,
) {
    let instance = get_update_instance(value);

    let v = Runtime::new().unwrap().block_on(put_instance(
        config,
        env.org_id.clone().unwrap().as_str(),
        instance_id,
        instance,
    ));

    match v {
        Ok(result) => {
            white_confirmation(&format!(
                "Instance update started for Instance Id: {}",
                result.instance_id.color(colors::sql_u()).bold()
            ));
        }
        Err(error) => eprintln!("Error updating instance: {}", error),
    };
}

fn create_new_instance(
    value: &InstanceSettings,
    config: &Configuration,
    env: Environment,
) -> Result<String, String> {
    let instance = get_create_instance(value);

    let v = Runtime::new().unwrap().block_on(create_instance(
        config,
        env.org_id.clone().unwrap().as_str(),
        instance,
    ));

    match v {
        Ok(result) => {
            white_confirmation(&format!(
                "Instance creation started for instance_name: {}",
                result.instance_name.color(colors::sql_u()).bold()
            ));

            Ok(result.instance_id)
        }
        Err(error) => {
            eprintln!("Error creating instance: {}", error);
            Err(error.to_string())
        }
    }
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
        extra_domains_rw: Some(instance_settings.extra_domains_rw.clone()),
        ip_allow_list: Some(instance_settings.ip_allow_list.clone()),
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
        extra_domains_rw: Some(instance_settings.extra_domains_rw.clone()),
        ip_allow_list: Some(instance_settings.ip_allow_list.clone()),
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

fn get_extensions(
    maybe_extensions: Option<HashMap<String, tembo_config::Extension>>,
) -> Vec<Extension> {
    let mut vec_extensions: Vec<Extension> = vec![];
    let mut vec_extension_location: Vec<ExtensionInstallLocation> = vec![];

    if let Some(extensions) = maybe_extensions {
        for (name, extension) in extensions.into_iter() {
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
    maybe_extensions: Option<HashMap<String, tembo_config::Extension>>,
) -> Vec<TrunkInstall> {
    let mut vec_trunk_installs: Vec<TrunkInstall> = vec![];

    if let Some(extensions) = maybe_extensions {
        for (_, extension) in extensions.into_iter() {
            if extension.trunk_project.is_some() {
                vec_trunk_installs.push(TrunkInstall {
                    name: extension.trunk_project.unwrap(),
                    version: Some(extension.trunk_project_version),
                });
            }
        }
    }
    vec_trunk_installs
}

fn merge_settings(base: &InstanceSettings, overlay: OverlayInstanceSettings) -> InstanceSettings {
    InstanceSettings {
        environment: base.environment.clone(), // Retain the base environment
        instance_name: base.instance_name.clone(), // Retain the base instance_name
        cpu: overlay.cpu.unwrap_or_else(|| base.cpu.clone()),
        memory: overlay.memory.unwrap_or_else(|| base.memory.clone()),
        storage: overlay.storage.unwrap_or_else(|| base.storage.clone()),
        replicas: overlay.replicas.unwrap_or(base.replicas),
        stack_type: overlay
            .stack_type
            .unwrap_or_else(|| base.stack_type.clone()),
        postgres_configurations: overlay
            .postgres_configurations
            .or_else(|| base.postgres_configurations.clone()),
        extensions: overlay.extensions.or_else(|| base.extensions.clone()),
        app_services: None,
        extra_domains_rw: overlay
            .extra_domains_rw
            .or_else(|| base.extra_domains_rw.clone()),
        ip_allow_list: overlay
            .ip_allow_list
            .or_else(|| base.extra_domains_rw.clone()),
    }
}

pub fn merge_instance_settings(
    base_settings: &HashMap<String, InstanceSettings>,
    overlay_file_path: &str,
) -> Result<HashMap<String, InstanceSettings>, Error> {
    let overlay_contents = fs::read_to_string(overlay_file_path)
        .with_context(|| format!("Couldn't read overlay file {}", overlay_file_path))?;
    let overlay_settings: HashMap<String, OverlayInstanceSettings> =
        toml::from_str(&overlay_contents).context("Unable to load data from the overlay config")?;

    let mut final_settings = base_settings.clone();
    for (key, overlay_value) in overlay_settings {
        if let Some(base_value) = base_settings.get(&key) {
            let merged_value = merge_settings(base_value, overlay_value);
            final_settings.insert(key, merged_value);
        }
    }

    Ok(final_settings)
}

pub fn set_instance_settings(
    base_settings: &mut HashMap<String, InstanceSettings>,
    set_arg: &str,
) -> Result<(), Error> {
    let (instance_name, setting_name, setting_value) = parse_set_arg(set_arg)?;

    if let Some(settings) = base_settings.get_mut(&instance_name) {
        match setting_name.as_str() {
            "instance_name" => settings.instance_name = setting_value,
            "cpu" => settings.cpu = setting_value,
            "memory" => settings.memory = setting_value,
            "storage" => settings.storage = setting_value,
            "replicas" => {
                settings.replicas = setting_value
                    .parse()
                    .map_err(|_| Error::msg("Invalid value for replicas"))?;
            }
            "stack_type" => settings.stack_type = setting_value,
            _ => {
                return Err(Error::msg(format!("Unknown setting: {}", setting_name)));
            }
        }
    } else {
        return Err(Error::msg("Instance not found"));
    }

    Ok(())
}

pub fn get_instance_settings(
    overlay_file_path: Option<String>,
    set_arg: Option<String>,
) -> Result<HashMap<String, InstanceSettings>, Error> {
    let mut base_path = FileUtils::get_current_working_dir();
    base_path.push_str("/tembo.toml");
    let base_contents = fs::read_to_string(&base_path)
        .with_context(|| format!("Couldn't read base file {}", base_path))?;

    let mut base_settings: HashMap<String, InstanceSettings> =
        toml::from_str(&base_contents).context("Unable to load data from the base config")?;

    if let Some(overlay_path) = overlay_file_path {
        let overlay_settings = merge_instance_settings(&base_settings, &overlay_path)?;
        base_settings.extend(overlay_settings);
    }

    if let Some(set_arg_str) = set_arg {
        set_instance_settings(&mut base_settings, &set_arg_str)?;
    }

    Ok(base_settings)
}

pub fn get_rendered_dockerfile(
    instance_settings: &InstanceSettings,
) -> Result<String, anyhow::Error> {
    // Include the Dockerfile template directly into the binary
    let contents = include_str!("../../tembo/Dockerfile.template");

    let mut tera = Tera::new("templates/**/*").unwrap();
    let _ = tera.add_raw_template("dockerfile", contents);
    let mut context = Context::new();

    let stack_type = ControllerStackType::from_str(instance_settings.stack_type.as_str())
        .unwrap_or(ControllerStackType::Standard);

    let stack = get_stack(stack_type);

    context.insert("stack_trunk_installs", &stack.trunk_installs);
    let extensions = instance_settings.extensions.clone().unwrap_or_default();
    context.insert("extensions", &extensions);

    let rendered_dockerfile = tera.render("dockerfile", &context).unwrap();

    Ok(rendered_dockerfile)
}

pub fn get_rendered_migrations_file(
    instance_settings: &InstanceSettings,
) -> Result<String, anyhow::Error> {
    // Include the migrations template directly into the binary
    let contents = include_str!("../../tembo/migrations.sql.template");

    let mut tera = Tera::new("templates/**/*")
        .map_err(|e| anyhow::anyhow!("Error initializing Tera: {}", e))?;
    tera.add_raw_template("migrations", contents)
        .map_err(|e| anyhow::anyhow!("Error adding raw template: {}", e))?;

    let mut context = Context::new();
    let stack_type = ControllerStackType::from_str(instance_settings.stack_type.as_str())
        .unwrap_or(ControllerStackType::Standard);

    let stack = get_stack(stack_type);

    context.insert("stack_extensions", &stack.extensions);
    context.insert("extensions", &instance_settings.extensions);

    let rendered_migrations = tera
        .render("migrations", &context)
        .map_err(|e| anyhow::anyhow!("Error rendering template: {}", e))?;

    Ok(rendered_migrations)
}

fn get_postgres_config(instance_settings: &InstanceSettings) -> String {
    let mut postgres_config = String::from("");
    let qoute_new_line = "\'\n";
    let equal_to_qoute = " = \'";
    let stack_type = ControllerStackType::from_str(instance_settings.stack_type.as_str())
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

    if instance_settings.postgres_configurations.is_some() {
        for (key, value) in instance_settings
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
    postgres_config
}

pub fn get_rendered_dockercompose(
    instance_settings: HashMap<String, InstanceSettings>,
) -> Result<String, anyhow::Error> {
    // Include the Dockerfile template directly into the binary
    let contents = include_str!("../../tembo/docker-compose.yml.template");

    let mut tera = Tera::new("templates/**/*").unwrap();
    let _ = tera.add_raw_template("docker-compose", contents);
    let mut context = Context::new();

    context.insert("instance_settings", &instance_settings);

    let rendered_dockerfile = tera.render("docker-compose", &context).unwrap();

    Ok(rendered_dockerfile)
}

fn construct_connection_string(info: ConnectionInfo) -> String {
    format!(
        "postgresql://{}:{}@{}:{}/{}",
        info.user,
        urlencoding::encode(&info.password),
        info.host,
        info.port,
        "postgres"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const ROOT_DIR: &str = env!("CARGO_MANIFEST_DIR");

    #[tokio::test]
    async fn merge_settings() -> Result<(), Box<dyn std::error::Error>> {
        std::env::set_current_dir(PathBuf::from(ROOT_DIR).join("examples").join("merge"))?;

        let overlay_config_path = PathBuf::from(ROOT_DIR)
            .join("examples")
            .join("merge")
            .join("overlay.toml");
        let overlay_config_str = overlay_config_path.to_str().unwrap();

        let merged_settings = get_instance_settings(Some(overlay_config_str.to_string()), None)?;
        if let Some(setting) = merged_settings.get("merge") {
            assert_ne!(
                setting.cpu, "0.25",
                "Default CPU setting was not overwritten"
            );
            assert_eq!(setting.replicas, 2, "Overlay Settings are not overwritten");
            assert_eq!(setting.storage, "10Gi", "Base Settings are not overwritten");
        } else {
            return Err("Merged setting key 'merge' not found".into());
        }

        Ok(())
    }

    #[tokio::test]
    async fn set_settings() -> Result<(), Box<dyn std::error::Error>> {
        std::env::set_current_dir(PathBuf::from(ROOT_DIR).join("examples").join("set"))?;

        let set_arg = "set.memory=2Gi";

        let final_settings = get_instance_settings(None, Some(set_arg.to_string()))?;

        if let Some(setting) = final_settings.get("set") {
            assert_eq!(
                setting.memory, "2Gi",
                "Memory setting was not correctly applied"
            );
        } else {
            return Err("Setting key 'defaults' not found".into());
        }

        Ok(())
    }
}
