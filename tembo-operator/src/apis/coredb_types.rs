use crate::extensions::types::ExtensionStatus;
use k8s_openapi::{
    api::core::v1::ResourceRequirements,
    apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::ObjectMeta},
};

use crate::{
    apis::postgres_parameters::{
        merge_pg_configs, MergeError, PgConfig, DISALLOWED_CONFIGS, MULTI_VAL_CONFIGS,
    },
    defaults,
    postgres_exporter::PostgresMetrics,
};
use kube::CustomResource;

use crate::extensions::types::{Extension, TrunkInstall, TrunkInstallStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct ServiceAccountTemplate {
    pub metadata: Option<ObjectMeta>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
#[allow(non_snake_case)]
pub struct Backup {
    #[serde(default = "defaults::default_destination_path")]
    pub destinationPath: Option<String>,
    #[serde(default = "defaults::default_encryption")]
    pub encryption: Option<String>,
    #[serde(default = "defaults::default_retention_policy")]
    pub retentionPolicy: Option<String>,
    #[serde(default = "defaults::default_backup_schedule")]
    pub schedule: Option<String>,
}

/// Generate the Kubernetes wrapper struct `CoreDB` from our Spec and Status struct
///
/// This provides a hook for generating the CRD yaml (in crdgen.rs)
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[cfg_attr(test, derive(Default))]
#[kube(kind = "CoreDB", group = "coredb.io", version = "v1alpha1", namespaced)]
#[kube(status = "CoreDBStatus", shortname = "cdb")]
#[allow(non_snake_case)]
pub struct CoreDBSpec {
    #[serde(default = "defaults::default_replicas")]
    pub replicas: i32,

    #[serde(default = "defaults::default_resources")]
    pub resources: ResourceRequirements,

    #[serde(default = "defaults::default_storage")]
    pub storage: Quantity,

    #[serde(default = "defaults::default_sharedir_storage")]
    pub sharedirStorage: Quantity,

    #[serde(default = "defaults::default_pkglibdir_storage")]
    pub pkglibdirStorage: Quantity,

    #[serde(default = "defaults::default_postgres_exporter_enabled")]
    pub postgresExporterEnabled: bool,

    #[serde(default = "defaults::default_image")]
    pub image: String,

    #[serde(default = "defaults::default_postgres_exporter_image")]
    pub postgresExporterImage: String,

    #[serde(default = "defaults::default_port")]
    pub port: i32,

    #[serde(default = "defaults::default_uid")]
    pub uid: i32,

    #[serde(default = "defaults::default_extensions")]
    pub extensions: Vec<Extension>,

    #[serde(default = "defaults::default_trunk_installs")]
    pub trunk_installs: Vec<TrunkInstall>,

    #[serde(default = "defaults::default_stop")]
    pub stop: bool,

    #[serde(default = "defaults::default_service_account_template")]
    pub serviceAccountTemplate: ServiceAccountTemplate,

    #[serde(default = "defaults::default_backup")]
    pub backup: Backup,

    pub metrics: Option<PostgresMetrics>,

    pub stack: Option<Stack>,
    // dynamic runtime configs
    pub runtime_config: Option<Vec<PgConfig>>,
    // configuration overrides, typically defined by the user
    pub override_configs: Option<Vec<PgConfig>>,
}

impl CoreDBSpec {
    // extracts all postgres configurations
    // configs can be defined in several different places (from a stack, user override, from an extension installation, user overrides, etc)
    pub fn get_pg_configs(&self) -> Result<Option<Vec<PgConfig>>, MergeError> {
        let stack_configs = self
            .stack
            .as_ref()
            .and_then(|s| s.postgres_config.clone())
            .unwrap_or_default();
        let runtime_configs = self.runtime_config.clone().unwrap_or_default();
        // TODO: configs that come with extension installation
        // e.g. let extension_configs = ...
        // these extensions could be set by the operator, or trunk + operator
        // trunk install pg_partman could come with something like `pg_partman_bgw.dbname = xxx`

        // handle merge of any of the settings that are multi-value.
        // e.g. stack defines shared_preload_libraries = pg_cron, then operator installs pg_stat_statements at runtime
        // we need to merge the two configs into one,  shared_preload_libraries = pg_cron, pg_stat_statements
        let mut merged_multival_configs: Vec<PgConfig> = Vec::new();
        for cfg_name in MULTI_VAL_CONFIGS {
            let merged_config = merge_pg_configs(&stack_configs, &runtime_configs, cfg_name)?;
            if let Some(merged_config) = merged_config {
                merged_multival_configs.push(merged_config);
            }
        }

        // Order matters - to ensure anything down stream does not have to worry about ordering,
        // set these into a BTreeSet now
        // 1. stack configs
        // 2. runtime configs
        // 3. merged multivals
        // 4. overrides
        let mut pg_configs: BTreeMap<String, PgConfig> = BTreeMap::new();

        for p in stack_configs {
            pg_configs.insert(p.name.clone(), p);
        }
        for p in runtime_configs {
            pg_configs.insert(p.name.clone(), p);
        }
        for p in merged_multival_configs {
            pg_configs.insert(p.name.clone(), p);
        }
        if let Some(override_configs) = &self.override_configs {
            for p in override_configs {
                pg_configs.insert(p.name.clone(), p.clone());
            }
        }

        // remove any configs that are not allowed
        for key in DISALLOWED_CONFIGS {
            pg_configs.remove(key);
        }

        if pg_configs.is_empty() {
            Ok(None)
        } else {
            Ok(Some(pg_configs.values().cloned().collect()))
        }
    }
}

/// The status object of `CoreDB`
#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[allow(non_snake_case)]
pub struct CoreDBStatus {
    pub running: bool,
    #[serde(default = "defaults::default_extensions_updating")]
    pub extensionsUpdating: bool,
    pub extensions: Option<Vec<ExtensionStatus>>,
    pub trunk_installs: Option<Vec<TrunkInstallStatus>>,
    pub storage: Option<Quantity>,
    pub resources: Option<ResourceRequirements>,
    pub runtime_config: Option<Vec<PgConfig>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Stack {
    pub name: String,
    // static configs defined in the tembo stack
    pub postgres_config: Option<Vec<PgConfig>>,
    // TODO: add other stack attributes as they are supported
}
