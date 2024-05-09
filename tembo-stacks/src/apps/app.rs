use anyhow::Error;
use lazy_static::lazy_static;
use std::collections::{BTreeMap, HashMap};
use tembo_controller::{
    apis::postgres_parameters::PgConfig,
    app_service::types::{AppService, EnvVar},
    extensions::types::{Extension, ExtensionInstallLocation, TrunkInstall},
};
use tracing::{info, instrument, warn};

use crate::apps::types::{App, AppConfig, AppType, MergedConfigs};

lazy_static! {
    pub static ref HTTP: App =
        serde_yaml::from_str(include_str!("http.yaml")).expect("http.yaml not found");
    pub static ref RESTAPI: App =
        serde_yaml::from_str(include_str!("restapi.yaml")).expect("restapi.yaml not found");
    pub static ref MQ: App =
        serde_yaml::from_str(include_str!("mq.yaml")).expect("mq.yaml not found");
    pub static ref EMBEDDINGS: App =
        serde_yaml::from_str(include_str!("embeddings.yaml")).expect("embeddings.yaml not found");
    pub static ref PGANALYZE: App =
        serde_yaml::from_str(include_str!("pganalyze.yaml")).expect("pganalyze.yaml not found");
}

// handling merging requirements coming from an App into the final
#[instrument(skip(user_apps, stack_apps, extensions, trunk_installs, pg_configs))]
pub fn merge_app_reqs(
    user_apps: Option<Vec<AppType>>,
    stack_apps: Option<Vec<AppService>>,
    extensions: Option<Vec<Extension>>,
    trunk_installs: Option<Vec<TrunkInstall>>,
    pg_configs: Option<Vec<PgConfig>>,
) -> Result<MergedConfigs, Error> {
    let mut fin_app_extensions: Vec<Extension> = vec![];
    let mut fin_app_trunk_installs: Vec<TrunkInstall> = vec![];
    let mut final_pg_configs: Vec<PgConfig> = vec![];

    let mut user_app_services: Vec<AppService> = vec![];
    // generates a Vec<AppService> from the user provided Apps
    if let Some(apps) = user_apps {
        for app in apps {
            match app {
                AppType::RestAPI(config) => {
                    // there is only 1 app_service in the restAPI
                    let mut restapi = RESTAPI.clone().app_services.unwrap().clone()[0].clone();
                    // if there are user provided configs, overwrite the defaults with them
                    if let Some(cfg) = config {
                        restapi = merge_app_configs(restapi, cfg);
                    };
                    user_app_services.push(restapi);
                    // restAPI only has app_service containers
                    // no extensions or trunk installs
                }
                AppType::HTTP(config) => {
                    let http = HTTP.clone();
                    let mut http_app_svc = http.app_services.unwrap()[0].clone();
                    if let Some(cfg) = config {
                        http_app_svc = merge_app_configs(http_app_svc, cfg);
                    };
                    user_app_services.push(http_app_svc);
                    if let Some(extensions) = http.extensions {
                        fin_app_extensions.extend(extensions);
                    }
                    if let Some(trunks) = http.trunk_installs {
                        fin_app_trunk_installs.extend(trunks);
                    }
                }
                AppType::MQ(config) => {
                    let mq = MQ.clone();
                    let mut mq_app_svc = mq.app_services.unwrap()[0].clone();
                    if let Some(cfg) = config {
                        mq_app_svc = merge_app_configs(mq_app_svc, cfg);
                    }
                    user_app_services.push(mq_app_svc);
                }
                AppType::Embeddings(config) => {
                    let embedding_app = EMBEDDINGS.clone();
                    // handle the app container from embeddings app
                    let mut embedding_app_svc = embedding_app.app_services.unwrap()[0].clone();
                    if let Some(cfg) = config {
                        embedding_app_svc = merge_app_configs(embedding_app_svc, cfg);
                    }
                    user_app_services.push(embedding_app_svc);
                    // handle extensions from embeddings app
                    if let Some(extensions) = embedding_app.extensions {
                        fin_app_extensions.extend(extensions);
                    }
                    // handle the trunk installs from embeddings app
                    if let Some(trunks) = embedding_app.trunk_installs {
                        fin_app_trunk_installs.extend(trunks);
                    }

                    if let Some(pg_cfg) = embedding_app.postgres_config {
                        final_pg_configs.extend(pg_cfg);
                    }
                }
                AppType::PgAnalyze(config) => {
                    // There is only 1 app_service in the pganalyze app
                    let pg_analyze = PGANALYZE.clone();
                    let mut pg_analyze_app_svc = pg_analyze.app_services.unwrap()[0].clone();
                    // If there are user provided configs, overwrite the defaults with them
                    if let Some(cfg) = config {
                        pg_analyze_app_svc = merge_app_configs(pg_analyze_app_svc, cfg);
                    }
                    user_app_services.push(pg_analyze_app_svc);
                    // Handle extensions from pganalyze app
                    if let Some(extensions) = pg_analyze.extensions {
                        fin_app_extensions.extend(extensions);
                    }
                    // Handle trunk installs from pganalyze app
                    if let Some(trunks) = pg_analyze.trunk_installs {
                        fin_app_trunk_installs.extend(trunks);
                    }
                    // Handle postgres_config from pganalyze app
                    if let Some(pg_cfg) = pg_analyze.postgres_config {
                        final_pg_configs.extend(pg_cfg);
                    }
                }
                AppType::Custom(custom_app) => {
                    user_app_services.push(custom_app);
                }
            }
        }
    }

    // merge stack apps into final app services
    // if there are any conflicts in naming, then we should return an error and notify user (4xx)
    let final_apps = match stack_apps {
        Some(mut stack_apps) => {
            for stack_app in stack_apps.iter_mut() {
                let Some(user_defined_configs) = user_app_services
                    .iter()
                    .find(|app| app.name == stack_app.name)
                else {
                    continue;
                };

                stack_app.resources = user_defined_configs.resources.clone();
                info!("Overwrote resources for stack app {}", stack_app.name);
            }

            let merged_apps = merge_apps(user_app_services, stack_apps)?;
            Some(merged_apps)
        }
        None => {
            if user_app_services.is_empty() {
                None
            } else {
                Some(user_app_services)
            }
        }
    };

    let mut final_extensions: Vec<Extension> = match extensions {
        Some(exts) => exts.clone(),
        None => vec![],
    };

    for app_ext in fin_app_extensions {
        for loc in app_ext.locations {
            final_extensions =
                merge_location_into_extensions(&app_ext.name, &loc, final_extensions);
        }
    }
    // final extensions
    let fe = if final_extensions.is_empty() {
        None
    } else {
        Some(final_extensions)
    };

    let final_trunks = match trunk_installs {
        Some(trunks) => Some(merge_trunk_installs(trunks, fin_app_trunk_installs)),
        None => {
            if fin_app_trunk_installs.is_empty() {
                None
            } else {
                Some(fin_app_trunk_installs)
            }
        }
    };

    // merge all the pg configs coming from Apps into the existing Instance configs
    let final_pg_configs = match pg_configs {
        Some(cfgs) => Some(merge_pg_configs(cfgs, final_pg_configs)),
        None => {
            if final_pg_configs.is_empty() {
                None
            } else {
                Some(final_pg_configs)
            }
        }
    };

    Ok(MergedConfigs {
        extensions: fe,
        trunk_installs: final_trunks,
        app_services: final_apps,
        pg_configs: final_pg_configs,
    })
}

// used for merging Vec of requested with Vec in Stack spec
#[instrument(skip(opt1, opt2))]
pub fn merge_options<T>(opt1: Option<Vec<T>>, opt2: Option<Vec<T>>) -> Option<Vec<T>>
where
    T: Clone,
{
    match (opt1, opt2) {
        (Some(mut vec1), Some(vec2)) => {
            vec1.extend(vec2);
            Some(vec1)
        }
        (Some(vec), None) | (None, Some(vec)) => Some(vec),
        (None, None) => None,
    }
}

#[instrument]
pub fn merge_location_into_extensions(
    extension_name: &str,
    new_location: &ExtensionInstallLocation,
    current_extensions: Vec<Extension>,
) -> Vec<Extension> {
    let mut new_extensions = current_extensions.clone();
    for extension in &mut new_extensions {
        // If the extension is already in the list
        if extension.name == extension_name {
            for location in &mut extension.locations {
                // If the location is already in the extension
                if location.database == new_location.database {
                    // Then replace it
                    *location = new_location.clone();
                    return new_extensions;
                }
            }
            // If we never found the location, append it to existing extension status
            extension.locations.push(new_location.clone());
            // Then sort the locations alphabetically by database and schema
            // sort locations by database and schema so the order is deterministic
            extension
                .locations
                .sort_by(|a, b| a.database.cmp(&b.database).then(a.schema.cmp(&b.schema)));
            return new_extensions;
        }
    }
    // If we never found the extension status, append it
    new_extensions.push(Extension {
        name: extension_name.to_string(),
        description: None,
        locations: vec![new_location.clone()],
    });
    // Then sort alphabetically by name
    new_extensions.sort_by(|a, b| a.name.cmp(&b.name));
    new_extensions
}

// merge user Apps and Stack Apps
// returns Err when there are any conflicts in naming
#[instrument]
fn merge_apps(
    user_apps: Vec<AppService>,
    stack_apps: Vec<AppService>,
) -> Result<Vec<AppService>, Error> {
    // users cannot override the names of any Apps originating from the Stack definition
    // create a set of the App names from Stack definitions
    // start w/ the Stack's Apps, and append any user apps assuming no conflicts
    let mut merged_apps: Vec<AppService> = stack_apps.clone();
    let mut stack_app_names = std::collections::HashSet::new();
    for app in &stack_apps {
        stack_app_names.insert(&app.name);
    }

    // users app names must also be unique across their defined Apps
    let mut user_app_names = std::collections::HashSet::new();
    for app in &user_apps {
        user_app_names.insert(&app.name);
    }
    if user_app_names.len() != user_apps.len() {
        return Err(Error::msg("Cannot have duplicate App names".to_string()));
    }
    // if we've reached this point, then user has no naming conflicts in their own Apps

    // check whether their names conflict with Stack App names
    for user_app in user_apps {
        // can expand this to validate any App attributes conflicts in the future
        if stack_app_names.contains(&user_app.name) {
            // TODO: do not allow user to override the appService that is defined in a Stack
            // need to find a way to report error
            warn!(
                "User App name: {} conflicts with Stack App name",
                user_app.name
            );
        } else {
            // no conflicts with Stack name, so we are good-to-go
            merged_apps.push(user_app);
        }
    }
    Ok(merged_apps)
}

// merges 2 vecs of PgConfigs
// except for multivalue configs, cfg2 overrides cfg2 if they match on name
#[instrument(skip(cfg1, cfg2))]
pub fn merge_pg_configs(cfg1: Vec<PgConfig>, cfg2: Vec<PgConfig>) -> Vec<PgConfig> {
    let mut map: BTreeMap<String, PgConfig> = BTreeMap::new();
    for cfg in cfg1 {
        map.insert(cfg.name.clone(), cfg);
    }
    for cfg in cfg2 {
        map.insert(cfg.name.clone(), cfg);
    }
    map.into_values().collect()
}

// merge two vecs of extensions
// vec2 overrides vec1 if they match on Extension.name
pub fn merge_extensions(vec1: Vec<Extension>, vec2: Vec<Extension>) -> Vec<Extension> {
    let mut map = HashMap::new();

    for ext in vec1 {
        map.insert(ext.name.clone(), ext);
    }

    for ext in vec2 {
        map.insert(ext.name.clone(), ext);
    }

    map.into_values().collect()
}

#[instrument(skip(vec1, vec2))]
pub fn merge_trunk_installs(vec1: Vec<TrunkInstall>, vec2: Vec<TrunkInstall>) -> Vec<TrunkInstall> {
    let mut map = HashMap::new();

    for ext in vec1 {
        map.insert(ext.name.clone(), ext);
    }

    for ext in vec2 {
        map.insert(ext.name.clone(), ext);
    }

    map.into_values().collect()
}

// handles overriding any default AppService configurations with user specified configurations
pub fn merge_app_configs(mut default_app: AppService, cfgs: AppConfig) -> AppService {
    // append override envs, if any, with the required env vars
    default_app.env = match (default_app.env, cfgs.env) {
        (Some(defaults), Some(overrides)) => {
            let envs = merge_env_defaults(defaults, overrides);
            Some(envs)
        }
        (None, Some(overrides)) => Some(overrides),
        (Some(defaults), None) => Some(defaults),
        (None, None) => None,
    };

    // override resources if present
    if let Some(resources) = cfgs.resources {
        default_app.resources = resources;
    }

    // override other configs as they become supported
    default_app
}

// overrides default env vars with user provided env vars
fn merge_env_defaults(defaults: Vec<EnvVar>, overrides: Vec<EnvVar>) -> Vec<EnvVar> {
    let mut default_map: HashMap<String, EnvVar> = defaults
        .into_iter()
        .map(|var| (var.name.clone(), var))
        .collect();
    for var in overrides {
        default_map.insert(var.name.clone(), var);
    }
    default_map.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_specs() {
        assert!(EMBEDDINGS.app_services.is_some());
        assert!(HTTP.app_services.is_some());
        assert!(MQ.app_services.is_some());
        assert!(PGANALYZE.app_services.is_some());
        assert!(RESTAPI.app_services.is_some());
    }

    #[test]
    fn test_pganalyze_spec() {
        let cfg = PGANALYZE.postgres_config.clone().unwrap();
        for c in cfg {
            if c.name == "log_line_prefix" {
                assert_eq!(c.value.to_string(), "'%m [%p] %q[user=%u,app=%a] ',db=%d")
            }
        }
    }

    #[test]
    fn test_merge_apps() {
        let user_apps = vec![
            AppService {
                name: "app1".to_string(),
                image: "user_image".to_string(),
                ..AppService::default()
            },
            AppService {
                name: "reserved_name_0".to_string(),
                image: "user_image".to_string(),
                ..AppService::default()
            },
        ];
        let stack_apps = vec![
            AppService {
                name: "reserved_name_0".to_string(),
                image: "stack_image".to_string(),
                ..AppService::default()
            },
            AppService {
                name: "reserved_name_1".to_string(),
                image: "stack_image".to_string(),
                ..AppService::default()
            },
        ];
        // stack_apps contains reserved_name_0, and user app also contained app with same name
        // this should ignore user request, and apply the Stack's app definition
        let merged_apps = merge_apps(user_apps, stack_apps.clone()).unwrap();
        assert_eq!(merged_apps.len(), 3);
        for app in merged_apps {
            if app.name == "reserved_name_0" {
                assert_eq!(app.image, "stack_image");
            }
        }

        let user_apps = vec![
            AppService {
                name: "sameName".to_string(),
                image: "image1".to_string(),
                ..AppService::default()
            },
            AppService {
                name: "sameName".to_string(),
                image: "image1".to_string(),
                ..AppService::default()
            },
        ];

        // there are duplicate names in the user Apps
        // this must error
        let merged_apps = merge_apps(user_apps, stack_apps.clone());
        assert!(merged_apps.is_err());

        let user_apps = vec![
            AppService {
                name: "app1".to_string(),
                image: "image1".to_string(),
                ..AppService::default()
            },
            AppService {
                name: "app2".to_string(),
                image: "image1".to_string(),
                ..AppService::default()
            },
        ];

        // no conflicts in names between user_apps and stack_apps
        // must succeed
        let merged_apps = merge_apps(user_apps, stack_apps).unwrap();
        assert_eq!(merged_apps.len(), 4);
    }

    #[test]
    fn test_merge_env_vars() {
        let e0 = EnvVar {
            name: "e0".to_string(),
            value: Some("e0".to_string()),
            value_from_platform: None,
        };
        let e1 = EnvVar {
            name: "e1".to_string(),
            value: Some("e1".to_string()),
            value_from_platform: None,
        };
        let e1_override = EnvVar {
            name: "e1".to_string(),
            value: Some("e1-override".to_string()),
            value_from_platform: None,
        };
        let e2 = EnvVar {
            name: "e2".to_string(),
            value: Some("e2".to_string()),
            value_from_platform: None,
        };
        let v0 = vec![e0.clone(), e1.clone()];
        let v1 = vec![e1.clone(), e1_override.clone(), e2.clone()];
        let merged = crate::apps::app::merge_env_defaults(v0.clone(), v1.clone());
        assert_eq!(merged.len(), 3);
        let merged_envs: HashMap<String, EnvVar> = merged
            .into_iter()
            .map(|var| (var.name.clone(), var))
            .collect();
        let same = merged_envs.get("e0").unwrap();
        assert_eq!(same.value, Some("e0".to_string()));
        let overridden = merged_envs.get("e1").unwrap();
        assert_eq!(overridden.value, Some("e1-override".to_string()));
        let same = merged_envs.get("e2").unwrap();
        assert_eq!(same.value, Some("e2".to_string()));
    }
}
