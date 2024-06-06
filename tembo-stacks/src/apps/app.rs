use anyhow::Error;
use lazy_static::lazy_static;
use std::collections::{BTreeMap, HashMap};
use tembo_controller::{
    apis::postgres_parameters::PgConfig,
    app_service::types::{AppService, EnvVar},
    extensions::types::{Extension, ExtensionInstallLocation, TrunkInstall},
};
use tracing::{instrument, warn};

use crate::apps::types::{App, AppConfig, AppType, MergedConfigs};

lazy_static! {
    pub static ref AI: App =
        serde_yaml::from_str(include_str!("ai.yaml")).expect("ai.yaml not found");
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
                AppType::AI(_config) => {
                    let ai = AI.clone();
                    let ai_app_svc = ai.app_services.unwrap()[0].clone();
                    // the AI appService is a proxy container to Tembo AI
                    // and its configuration should not be modified
                    user_app_services.push(ai_app_svc);
                    if let Some(extensions) = ai.extensions {
                        fin_app_extensions.extend(extensions);
                    }
                    if let Some(trunks) = ai.trunk_installs {
                        fin_app_trunk_installs.extend(trunks);
                    }
                }
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
    let final_apps = match stack_apps {
        Some(s_apps) => {
            let merged_apps = merge_apps(user_app_services, s_apps)?;
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
#[instrument]
fn merge_apps(
    user_apps: Vec<AppService>,
    stack_apps: Vec<AppService>,
) -> Result<Vec<AppService>, Error> {
    // when user provides configuration for app with same name as another app,
    // the user provided configuration overrides the existing configuration
    let mut final_apps: HashMap<String, AppService> = HashMap::new();
    for app in stack_apps {
        final_apps.insert(app.name.clone(), app);
    }
    for app in user_apps {
        final_apps.insert(app.name.clone(), app);
    }
    Ok(final_apps.into_values().collect())
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
    use crate::apps::types::AppConfig;
    use tembo_controller::app_service::types::EnvVar;
    #[test]
    fn test_merge_app_reqs() {
        let app_config = AppConfig {
            env: Some(vec![
                EnvVar {
                    name: "APP_ENV".to_string(),
                    value: Some("user".to_string()),
                    value_from_platform: None,
                },
                EnvVar {
                    name: "TMPDIR".to_string(),
                    value: Some("/custom_dir".to_string()),
                    value_from_platform: None,
                },
            ]),
            resources: None,
        };
        let user_embedding_app = AppType::Embeddings(Some(app_config));
        let user_apps = vec![user_embedding_app];
        let stack_apps = vec![AppService {
            name: "embeddings".to_string(),
            env: Some(vec![EnvVar {
                name: "APP_ENV".to_string(),
                value: Some("stack".to_string()),
                value_from_platform: None,
            }]),
            ..AppService::default()
        }];
        let merged_configs: MergedConfigs =
            merge_app_reqs(Some(user_apps), Some(stack_apps), None, None, None).unwrap();
        let app = merged_configs.app_services.unwrap()[0].clone();
        let mut to_find = 2;
        // 3 embedding app defaults + 1 custom
        println!("{:?}", app.env.as_ref().unwrap());
        assert_eq!(app.env.as_ref().unwrap().len(), 4);
        for e in app.env.unwrap() {
            match e.name.as_str() {
                // custom env var is found
                "APP_ENV" => {
                    assert_eq!(e.value.unwrap(), "user".to_string());
                    to_find -= 1;
                }
                // overridden TMPDIR value is found
                "TMPDIR" => {
                    assert_eq!(e.value.unwrap(), "/custom_dir".to_string());
                    to_find -= 1;
                }
                _ => {}
            }
        }
        assert_eq!(to_find, 0);

        // validate metrics end up in final_app
        let metrics = app.metrics.expect("metrics not found");
        assert_eq!(metrics.path, "/metrics".to_string());
        assert_eq!(metrics.port, 3000);
    }

    #[test]
    fn test_app_specs() {
        assert!(EMBEDDINGS.app_services.is_some());
        assert!(HTTP.app_services.is_some());
        assert!(MQ.app_services.is_some());
        assert!(PGANALYZE.app_services.is_some());
        assert!(RESTAPI.app_services.is_some());
        assert!(AI.app_services.is_some());
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
                name: "app2".to_string(),
                image: "user_image".to_string(),
                ..AppService::default()
            },
        ];
        let stack_apps = vec![
            AppService {
                name: "app1".to_string(),
                image: "stack_image".to_string(),
                ..AppService::default()
            },
            AppService {
                name: "app3".to_string(),
                image: "stack_image".to_string(),
                ..AppService::default()
            },
        ];
        // app1 should be overriten with the user provided image
        let merged_apps = merge_apps(user_apps, stack_apps.clone()).unwrap();
        assert_eq!(merged_apps.len(), 3);
        for app in merged_apps {
            if app.name == "app1" {
                assert_eq!(app.image, "user_image");
            }
            // reserved_name_1 should not be overriten
            if app.name == "reserved_name_1" {
                assert_eq!(app.image, "stack_image");
            }
        }
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
