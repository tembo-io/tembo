use anyhow::Context as AnyhowContext;
use anyhow::Error;
use clap::Args;
use colorful::Colorful;
use controller::apis::postgres_parameters::ConfigValue as ControllerConfigValue;
use controller::apis::postgres_parameters::PgConfig as ControllerPgConfig;
use controller::app_service::types::AppService;
use controller::app_service::types::EnvVar;
use controller::extensions::types::Extension as ControllerExtension;
use controller::extensions::types::ExtensionInstallLocation as ControllerExtensionInstallLocation;
use controller::extensions::types::TrunkInstall as ControllerTrunkInstall;
use controller::stacks::get_stack;
use controller::stacks::types::StackType as ControllerStackType;
use itertools::Itertools;
use log::info;
use spinoff::spinners;
use spinoff::Spinner;
use std::fmt::Write;
use std::{
    collections::HashMap,
    fs::{self},
    str::FromStr,
    thread::sleep,
    time::Duration,
};
use tembo_stacks::apps::app::merge_app_reqs;
use tembo_stacks::apps::app::merge_options;
use tembo_stacks::apps::types::MergedConfigs;
use temboclient::apis::instance_api::patch_instance;
use temboclient::models::ExtensionStatus;
use temboclient::models::Instance;
use temboclient::models::PatchInstance;
use temboclient::{
    apis::{
        configuration::Configuration,
        instance_api::{create_instance, get_all, get_instance},
    },
    models::{
        ConnectionInfo, Cpu, CreateInstance, Extension, ExtensionInstallLocation, Memory, PgConfig,
        StackType, State, Storage, TrunkInstall,
    },
};
use tembodataclient::apis::secrets_api::get_secret_v1;
use tokio::runtime::Runtime;
use toml::Value;

use crate::cli::docker::Docker;
use crate::cli::file_utils::FileUtils;
use crate::cli::sqlx_utils::SqlxUtils;
use crate::cli::tembo_config;
use crate::cli::tembo_config::InstanceSettings;
use crate::cli::tembo_config::Library;
use crate::cli::tembo_config::OverlayInstanceSettings;
use crate::cli::tembo_config::TrunkProject;
use crate::tui;
use crate::{
    cli::context::{get_current_context, Environment, Profile, Target},
    tui::{clean_console, colors, instance_started, white_confirmation},
};
use tera::{Context, Tera};

const DOCKERFILE_NAME: &str = "Dockerfile";
const DOCKERCOMPOSE_NAME: &str = "docker-compose.yml";
const POSTGRESCONF_NAME: &str = "postgres.conf";
const MAX_INT32: i32 = 2147483647;

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
                tui::error(&format!("{}", error));
                return Ok(());
            }
        }
    }

    Ok(())
}

fn docker_apply(
    verbose: bool,
    mut instance_settings: HashMap<String, InstanceSettings>,
) -> Result<(), anyhow::Error> {
    Docker::installed_and_running()?;

    Docker::docker_compose_down(false)?;

    let mut final_instance_settings: HashMap<String, InstanceSettings> = Default::default();

    for (_key, instance_setting) in instance_settings.iter_mut() {
        let final_instance_setting = docker_apply_instance(verbose, instance_setting.to_owned())?;
        final_instance_settings.insert(
            final_instance_setting.instance_name.clone(),
            final_instance_setting,
        );
    }

    let rendered_dockercompose: String =
        get_rendered_dockercompose(final_instance_settings.clone())?;

    FileUtils::create_file(
        DOCKERCOMPOSE_NAME.to_string(),
        DOCKERCOMPOSE_NAME.to_string(),
        rendered_dockercompose,
        true,
    )?;

    Docker::docker_compose_up(verbose)?;

    // Allows DB instance to be ready before running CREATE EXTENSION script
    sleep(Duration::from_secs(5));

    let port = 5432;

    for (_key, instance_setting) in final_instance_settings.clone().iter() {
        let instance_name = &instance_setting.instance_name;

        let mut sp = Spinner::new(spinners::Dots, "Creating extensions", spinoff::Color::White);

        for ext in instance_setting.final_extensions.clone().unwrap().iter() {
            let query = &format!("CREATE EXTENSION IF NOT EXISTS {} CASCADE", ext.name);

            Runtime::new().unwrap().block_on(SqlxUtils::execute_sql(
                instance_name.to_string(),
                query.to_string(),
            ))?;
        }

        sp.stop_with_message(&format!(
            "{} {}",
            "✓".color(colors::indicator_good()).bold(),
            format!("Extensions created for instance {}", instance_name)
                .color(colorful::Color::White)
                .bold()
        ));

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
    mut instance_setting: InstanceSettings,
) -> Result<InstanceSettings, anyhow::Error> {
    FileUtils::create_dir(
        instance_setting.instance_name.clone(),
        instance_setting.instance_name.clone(),
    )?;

    let stack_type = ControllerStackType::from_str(instance_setting.stack_type.as_str())
        .unwrap_or(ControllerStackType::Standard);
    let stack = get_stack(stack_type);

    let extensions = merge_options(
        stack.extensions.clone(),
        Some(get_extensions_controller(
            instance_setting.extensions.clone(),
        )),
    );
    let trunk_installs = merge_options(
        stack.trunk_installs.clone(),
        Some(get_trunk_installs_controller(
            instance_setting.extensions.clone(),
        )),
    );

    let MergedConfigs {
        extensions,
        trunk_installs,
        app_services,
        pg_configs,
    } = merge_app_reqs(
        instance_setting.app_services.clone(),
        stack.app_services.clone(),
        extensions,
        trunk_installs,
        stack.postgres_config,
    )?;

    let rendered_dockerfile: String = get_rendered_dockerfile(&trunk_installs)?;

    FileUtils::create_file(
        DOCKERFILE_NAME.to_string(),
        instance_setting.instance_name.clone() + "/" + DOCKERFILE_NAME,
        rendered_dockerfile,
        true,
    )?;

    instance_setting.final_extensions = extensions;

    FileUtils::create_file(
        POSTGRESCONF_NAME.to_string(),
        instance_setting.instance_name.clone() + "/" + POSTGRESCONF_NAME,
        get_postgres_config(
            instance_setting.final_extensions.as_ref(),
            instance_setting.postgres_configurations.clone(),
            &pg_configs,
        )?,
        true,
    )?;

    Docker::build(instance_setting.instance_name.clone(), verbose)?;

    process_app_services(app_services, &mut instance_setting);

    Ok(instance_setting)
}

fn process_app_services(
    app_services: Option<Vec<AppService>>,
    instance_setting: &mut InstanceSettings,
) {
    let local_pgrst_db_uri = format!(
        "postgresql://postgres:postgres@{}:5432/postgres",
        &instance_setting.instance_name
    );
    const PGRST_DB_URI_NAME: &str = "PGRST_DB_URI";
    if app_services.is_some() {
        let mut controller_app_svcs: HashMap<String, AppService> = Default::default();
        for cas in app_services.unwrap().iter_mut() {
            if let Some(env_vars) = cas.env.as_mut() {
                let maybe_env_var = env_vars
                    .iter_mut()
                    .find_or_first(|f| f.name == *PGRST_DB_URI_NAME);

                if let Some(env_var) = maybe_env_var {
                    if env_var.value.is_none() {
                        cas.env.as_mut().unwrap().push(EnvVar {
                            name: PGRST_DB_URI_NAME.to_string(),
                            value: Some(local_pgrst_db_uri.to_string()),
                            value_from_platform: None,
                        });
                    }
                }
            }
            controller_app_svcs.insert(cas.name.clone(), cas.to_owned());
        }

        instance_setting.controller_app_services = Some(controller_app_svcs);
    }
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
        base_path: profile.get_tembo_host(),
        bearer_access_token: Some(profile.tembo_access_token.clone()),
        ..Default::default()
    };

    let maybe_instance = get_maybe_instance(&instance_settings.instance_name, &config, env)?;

    let instance_id;

    if let Some(env_instance) = &maybe_instance {
        instance_id = Some(env_instance.clone().instance_id);
        update_existing_instance(env_instance, instance_settings, &config, env)?;
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
                instance_id,
                connection_info,
                env.clone(),
            )?;

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
    instance_id: Option<String>,
    connection_info: Option<Box<ConnectionInfo>>,
    env: Environment,
) -> Result<ConnectionInfo, anyhow::Error> {
    let dataplane_config = tembodataclient::apis::configuration::Configuration {
        base_path: profile.get_tembo_data_host(),
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
        println!();
        return Err(Error::msg("Error fetching instance credentials!"));
    }

    let mut conn_info = *connection_info.unwrap();

    let map = result.as_ref().unwrap();

    conn_info.user = map.get("username").unwrap().to_string();
    conn_info.password = map.get("password").unwrap().to_string();

    Ok(conn_info)
}

pub fn get_maybe_instance(
    instance_name: &str,
    config: &Configuration,
    env: &Environment,
) -> Result<Option<Instance>, anyhow::Error> {
    let v = Runtime::new()
        .unwrap()
        .block_on(get_all(config, env.org_id.clone().unwrap().as_str()));

    match v {
        Ok(result) => {
            let maybe_instance = result
                .iter()
                .find(|instance| instance.instance_name == instance_name);

            if let Some(instance) = maybe_instance {
                return Ok(Some(instance.clone()));
            }
        }
        Err(error) => eprintln!("Error getting instance: {}", error),
    };
    Ok(None)
}

pub fn get_instance_id(
    instance_name: &str,
    config: &Configuration,
    env: &Environment,
) -> Result<Option<String>, anyhow::Error> {
    let maybe_instance = get_maybe_instance(instance_name, config, env)?;

    if let Some(instance) = maybe_instance {
        return Ok(Some(instance.instance_id));
    }
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
    instance: &Instance,
    value: &InstanceSettings,
    config: &Configuration,
    env: &Environment,
) -> Result<(), anyhow::Error> {
    let maybe_instance = get_patch_instance(instance, value);

    match maybe_instance {
        Ok(update_instance) => {
            let v = Runtime::new().unwrap().block_on(patch_instance(
                config,
                env.org_id.clone().unwrap().as_str(),
                &instance.instance_id,
                update_instance,
            ));

            match v {
                Ok(result) => {
                    white_confirmation(&format!(
                        "Instance update started for Instance Id: {}",
                        result.instance_id.color(colors::sql_u()).bold()
                    ));
                }
                Err(error) => {
                    return Err(Error::msg(format!("Error updating instance: {}", error)))
                }
            };
        }
        Err(error) => return Err(Error::msg(format!("Error updating instance: {}", error))),
    }
    Ok(())
}

fn create_new_instance(
    value: &InstanceSettings,
    config: &Configuration,
    env: Environment,
) -> Result<String, String> {
    let maybe_instance = get_create_instance(value);

    match maybe_instance {
        Ok(instance) => {
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
        Err(error) => {
            eprintln!("Error creating instance: {}", error);
            Err(error.to_string())
        }
    }
}

fn get_create_instance(
    instance_settings: &InstanceSettings,
) -> Result<CreateInstance, anyhow::Error> {
    return Ok(CreateInstance {
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
        extensions: Some(Some(get_extensions(
            instance_settings.extensions.clone(),
            &None,
        )?)),
        extra_domains_rw: Some(instance_settings.extra_domains_rw.clone()),
        ip_allow_list: Some(instance_settings.ip_allow_list.clone()),
        trunk_installs: Some(Some(get_trunk_installs(
            instance_settings.extensions.clone(),
        ))),
        postgres_configs: Some(Some(get_postgres_config_cloud(instance_settings)?)),
        pg_version: Some(instance_settings.pg_version.into()),
    });
}

fn get_patch_instance(
    instance: &Instance,
    instance_settings: &InstanceSettings,
) -> Result<PatchInstance, anyhow::Error> {
    return Ok(PatchInstance {
        cpu: Some(Some(Cpu::from_str(instance_settings.cpu.as_str()).unwrap())),
        memory: Some(Some(
            Memory::from_str(instance_settings.memory.as_str()).unwrap(),
        )),
        environment: Some(Some(
            temboclient::models::Environment::from_str(instance_settings.environment.as_str())
                .unwrap(),
        )),
        storage: Some(Some(
            Storage::from_str(instance_settings.storage.as_str()).unwrap(),
        )),
        replicas: Some(Some(instance_settings.replicas)),
        app_services: None,
        connection_pooler: None,
        extensions: Some(Some(get_extensions(
            instance_settings.extensions.clone(),
            &instance.extensions,
        )?)),
        extra_domains_rw: Some(instance_settings.extra_domains_rw.clone()),
        ip_allow_list: Some(instance_settings.ip_allow_list.clone()),
        trunk_installs: Some(Some(get_trunk_installs(
            instance_settings.extensions.clone(),
        ))),
        postgres_configs: Some(Some(get_postgres_config_cloud(instance_settings)?)),
    });
}

fn get_postgres_config_cloud(
    instance_settings: &InstanceSettings,
) -> Result<Vec<PgConfig>, anyhow::Error> {
    let mut pg_configs: Vec<PgConfig> = vec![];

    if instance_settings.postgres_configurations.is_some() {
        for (key, value) in instance_settings
            .postgres_configurations
            .as_ref()
            .unwrap()
            .iter()
        {
            match value {
                Value::String(string) => pg_configs.push(PgConfig {
                    name: key.to_owned(),
                    value: string.to_owned(),
                }),
                Value::Table(table) => {
                    for (inner_key, value) in table {
                        let value = match value {
                            Value::String(str) => str.to_owned(),
                            other => other.to_string(),
                        };

                        pg_configs.push(PgConfig {
                            name: format!("{key}.{inner_key}"),
                            value,
                        })
                    }
                }
                Value::Integer(int) => pg_configs.push(PgConfig {
                    name: key.to_owned(),
                    value: int.to_string(),
                }),
                Value::Boolean(bool) => pg_configs.push(PgConfig {
                    name: key.to_owned(),
                    value: bool.to_string(),
                }),
                Value::Datetime(dttm) => pg_configs.push(PgConfig {
                    name: key.to_owned(),
                    value: dttm.to_string(),
                }),
                Value::Float(fl) => pg_configs.push(PgConfig {
                    name: key.to_owned(),
                    value: fl.to_string(),
                }),
                _ => {
                    return Err(Error::msg(format!(
                        "Error processing postgres_config: {}",
                        key.to_owned()
                    )));
                }
            }
        }
    }

    Ok(pg_configs)
}

fn get_extensions(
    maybe_extensions: Option<HashMap<String, tembo_config::Extension>>,
    maybe_existing_extensions: &Option<Option<Vec<ExtensionStatus>>>,
) -> Result<Vec<Extension>, anyhow::Error> {
    let mut vec_extensions: Vec<Extension> = vec![];

    if let Some(extensions) = maybe_extensions {
        for (name, extension) in extensions.into_iter() {
            let mut version = Runtime::new().unwrap().block_on(get_extension_version(
                name.clone(),
                extension.clone().version,
            ))?;

            // Handle extension version change during an update
            if let Some(Some(existing_extensions)) = maybe_existing_extensions {
                let extension_mismatch = existing_extensions
                    .iter()
                    .find(|f| f.name == name && f.locations[0].version != Some(version.clone()));

                if extension_mismatch.is_some() {
                    if extension.version.is_some() {
                        return Err(Error::msg(format!(
                            "Current version of extension {} installed is different than version specified in tembo.toml",
                            name
                        )));
                    } else {
                        let version_error = format!(
                            "Current version of extension {} installed is different than version on trunk",
                            name);
                        let ext_locations = extension_mismatch.unwrap().locations.clone();
                        if !ext_locations.is_empty() {
                            if let Some(existing_version) = ext_locations[0].clone().version {
                                version = existing_version
                            } else {
                                return Err(Error::msg(version_error));
                            }
                        } else {
                            return Err(Error::msg(version_error));
                        }
                    }
                }
            }

            let vec_extension_location: Vec<ExtensionInstallLocation> =
                vec![ExtensionInstallLocation {
                    database: Some("postgres".to_string()),
                    schema: None,
                    version: Some(version),
                    enabled: extension.enabled,
                }];

            vec_extensions.push(Extension {
                name: name.to_owned(),
                description: None,
                locations: vec_extension_location.clone(),
            });
        }
    }

    Ok(vec_extensions)
}

fn get_extensions_controller(
    maybe_extensions: Option<HashMap<String, tembo_config::Extension>>,
) -> Vec<ControllerExtension> {
    let mut vec_extensions: Vec<ControllerExtension> = vec![];
    let mut vec_extension_location: Vec<ControllerExtensionInstallLocation> = vec![];

    if let Some(extensions) = maybe_extensions {
        for (name, extension) in extensions.into_iter() {
            vec_extension_location.push(ControllerExtensionInstallLocation {
                database: String::new(),
                schema: None,
                version: None,
                enabled: extension.enabled,
            });

            vec_extensions.push(ControllerExtension {
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

fn get_trunk_installs_controller(
    maybe_extensions: Option<HashMap<String, tembo_config::Extension>>,
) -> Vec<ControllerTrunkInstall> {
    let mut vec_trunk_installs: Vec<ControllerTrunkInstall> = vec![];

    if let Some(extensions) = maybe_extensions {
        for (_, extension) in extensions.into_iter() {
            if extension.trunk_project.is_some() {
                vec_trunk_installs.push(ControllerTrunkInstall {
                    name: extension.trunk_project.unwrap(),
                    version: extension.trunk_project_version,
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
        final_extensions: None,
        app_services: None,
        controller_app_services: None,
        extra_domains_rw: overlay
            .extra_domains_rw
            .or_else(|| base.extra_domains_rw.clone()),
        ip_allow_list: overlay
            .ip_allow_list
            .or_else(|| base.extra_domains_rw.clone()),
        pg_version: overlay.pg_version.unwrap_or(base.pg_version),
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
    trunk_installs: &Option<Vec<ControllerTrunkInstall>>,
) -> Result<String, anyhow::Error> {
    // Include the Dockerfile template directly into the binary
    let contents = include_str!("../../tembo/Dockerfile.template");

    let mut tera = Tera::new("templates/**/*").unwrap();
    let _ = tera.add_raw_template("dockerfile", contents);
    let mut context = Context::new();

    context.insert("trunk_installs", &trunk_installs);

    let rendered_dockerfile = tera.render("dockerfile", &context).unwrap();

    Ok(rendered_dockerfile)
}

fn get_postgres_config(
    extensions: Option<&Vec<ControllerExtension>>,
    instance_ps_config: Option<HashMap<String, Value>>,
    postgres_configs: &std::option::Option<Vec<ControllerPgConfig>>,
) -> Result<String, Error> {
    let mut postgres_config = String::from("");
    let mut shared_preload_libraries: Vec<Library> = Vec::new();

    if let Some(ps_config) = postgres_configs {
        for p_config in ps_config.clone().into_iter() {
            match p_config.name.as_str() {
                "shared_preload_libraries" => {
                    match p_config.value {
                        ControllerConfigValue::Single(val) => {
                            shared_preload_libraries.push(Library {
                                name: val.to_string(),
                                priority: MAX_INT32,
                            });
                        }
                        ControllerConfigValue::Multiple(vals) => {
                            for val in vals {
                                shared_preload_libraries.push(Library {
                                    name: val.to_string(),
                                    priority: MAX_INT32,
                                });
                            }
                        }
                    };
                }
                _ => {
                    match p_config.value {
                        ControllerConfigValue::Single(val) => {
                            let _ =
                                writeln!(postgres_config, "{} = '{val}'", p_config.name.as_str());
                        }
                        ControllerConfigValue::Multiple(vals) => {
                            for val in vals {
                                let _ = writeln!(
                                    postgres_config,
                                    "{} = '{val}'",
                                    p_config.name.as_str()
                                );
                            }
                        }
                    };
                }
            }
        }
    }

    if let Some(ps_config) = instance_ps_config {
        for (key, value) in ps_config.iter() {
            match key.as_str() {
                "shared_preload_libraries" => {
                    shared_preload_libraries.push(Library {
                        name: value.as_str().unwrap().to_string(),
                        priority: MAX_INT32,
                    });
                }
                _ => {
                    if value.is_str() {
                        let _ = writeln!(postgres_config, "{} = '{value}'", key.as_str());
                    }
                    if value.is_table() {
                        for row in value.as_table().iter() {
                            for (t, v) in row.iter() {
                                let _ = writeln!(
                                    postgres_config,
                                    "{}.{} = '{}'",
                                    key.as_str(),
                                    t.as_str(),
                                    v.as_str().unwrap()
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    let maybe_final_loadable_libs = Runtime::new().unwrap().block_on(get_loadable_libraries(
        shared_preload_libraries.clone(),
        extensions,
    ));

    match maybe_final_loadable_libs {
        Ok(l) => {
            let config = l
                .into_iter()
                .unique_by(|f| f.name.clone())
                .sorted_by_key(|s| (s.priority, s.name.clone()))
                .map(|x| x.name.to_string() + ",")
                .collect::<String>();

            let final_libs = config.split_at(config.len() - 1);

            let libs_config = format!("shared_preload_libraries = '{}'", final_libs.0);

            postgres_config.push_str(&libs_config);
        }
        Err(error) => {
            return Err(error);
        }
    }

    Ok(postgres_config)
}

async fn get_loadable_libraries(
    mut shared_preload_libraries: Vec<Library>,
    maybe_extensions: Option<&Vec<ControllerExtension>>,
) -> Result<Vec<Library>, anyhow::Error> {
    if let Some(extensions) = maybe_extensions {
        for ext in extensions.iter() {
            let trunk_projects = get_trunk_projects(&ext.name).await?;

            // If more than 1 trunk_project is returned then skip adding "shared_preload_libraries"
            if trunk_projects.len() > 1 {
                return Ok(shared_preload_libraries);
            }

            for trunk_project in trunk_projects.iter() {
                if let Some(extensions) = trunk_project.extensions.as_ref() {
                    for trunk_extension in extensions.iter() {
                        if trunk_extension.extension_name != ext.name {
                            continue;
                        }
                        if let Some(loadable_lib) = trunk_extension.loadable_libraries.as_ref() {
                            for loadable_lib in loadable_lib.iter() {
                                shared_preload_libraries.push(Library {
                                    name: loadable_lib.library_name.clone(),
                                    priority: loadable_lib.priority,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(shared_preload_libraries)
}

async fn get_extension_version(
    name: String,
    maybe_version: Option<String>,
) -> Result<Option<String>, anyhow::Error> {
    if let Some(version) = maybe_version {
        return Ok(Some(version));
    }

    let trunk_projects = get_trunk_projects(&name).await?;

    // If trunk projects returned is not exactly 1 then skip getting version
    if trunk_projects.len() != 1 {
        return Ok(None);
    }

    let trunk_project = &trunk_projects[0];

    if let Some(extensions) = trunk_project.extensions.as_ref() {
        for trunk_extension in extensions.iter() {
            if trunk_extension.extension_name != name {
                continue;
            }
            return Ok(Some(trunk_extension.version.clone()));
        }
    }
    Ok(None)
}

async fn get_trunk_projects(name: &String) -> Result<Vec<TrunkProject>, Error> {
    let trunk_projects_url = "https://registry.pgtrunk.io/api/v1/trunk-projects?extension-name=";
    let response = reqwest::get(format!("{}{}", trunk_projects_url, name))
        .await?
        .text()
        .await?;
    let trunk_projects: Vec<TrunkProject> = serde_json::from_str(&response)?;
    Ok(trunk_projects)
}

pub fn get_rendered_dockercompose(
    instance_settings: HashMap<String, InstanceSettings>,
) -> Result<String, anyhow::Error> {
    // Include the docker-compose template directly into the binary
    let contents = include_str!("../../tembo/docker-compose.yml.template");

    let mut tera = Tera::new("templates/**/*").unwrap();
    let _ = tera.add_raw_template("docker-compose", contents);
    let mut context = Context::new();

    context.insert("instance_settings", &instance_settings);

    let rendered_dockercompose = tera.render("docker-compose", &context).unwrap();

    Ok(rendered_dockercompose)
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
