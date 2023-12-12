use crate::{
    apis::postgres_parameters::{
        merge_pg_configs, ConfigValue, MergeError, PgConfig, DISALLOWED_CONFIGS, MULTI_VAL_CONFIGS,
    },
    app_service::types::AppService,
    defaults,
    extensions::types::{Extension, ExtensionStatus, TrunkInstall, TrunkInstallStatus},
    postgres_exporter::PostgresMetrics,
};

use k8s_openapi::{
    api::core::v1::ResourceRequirements,
    apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::ObjectMeta},
};

use crate::cloudnativepg::poolers::{PoolerPgbouncerPoolMode, PoolerTemplateSpecContainersResources};
use chrono::{DateTime, Utc};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use tracing::error;
use utoipa::ToSchema;

#[derive(Clone, Default, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Stack {
    pub name: String,
    pub postgres_config: Option<Vec<PgConfig>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
pub struct ServiceAccountTemplate {
    pub metadata: Option<ObjectMeta>,
}

/// S3Credentials is the type for the credentials to be used to upload files to S3.
/// It can be provided in two alternative ways:
/// * explicitly passing accessKeyId and secretAccessKey
/// * inheriting the role from the pod environment by setting inheritFromIAMRole to true
#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema, ToSchema)]
pub struct S3Credentials {
    /// The reference to the access key id
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "accessKeyId")]
    pub access_key_id: Option<S3CredentialsAccessKeyId>,

    /// Use the role based authentication without providing explicitly the keys.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "inheritFromIAMRole"
    )]
    pub inherit_from_iam_role: Option<bool>,

    /// The reference to the secret containing the region name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<S3CredentialsRegion>,

    /// The reference to the secret access key
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "secretAccessKey")]
    pub secret_access_key: Option<S3CredentialsSecretAccessKey>,

    /// The references to the session key
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "sessionToken")]
    pub session_token: Option<S3CredentialsSessionToken>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema, ToSchema)]
pub struct S3CredentialsAccessKeyId {
    pub key: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema, ToSchema)]
pub struct S3CredentialsRegion {
    pub key: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema, ToSchema)]
pub struct S3CredentialsSecretAccessKey {
    pub key: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema, ToSchema)]
pub struct S3CredentialsSessionToken {
    pub key: String,
    pub name: String,
}

/// CoreDB Backup configuration
#[derive(Deserialize, Serialize, Clone, Debug, Default, JsonSchema, ToSchema)]
#[allow(non_snake_case)]
pub struct Backup {
    /// The S3 bucket path to store backups in
    #[serde(default = "defaults::default_destination_path")]
    pub destinationPath: Option<String>,

    /// The S3 encryption algorithm to use for backups
    #[serde(default = "defaults::default_encryption")]
    pub encryption: Option<String>,

    /// The number of days to retain backups for
    #[serde(default = "defaults::default_retention_policy")]
    pub retentionPolicy: Option<String>,

    /// The backup schedule set with cron syntax
    #[serde(default = "defaults::default_backup_schedule")]
    pub schedule: Option<String>,

    /// The S3 compatable endpoint URL
    #[serde(default, rename = "endpointURL")]
    pub endpoint_url: Option<String>,

    /// The S3 credentials to use for backups (if not using IAM Role)
    #[serde(default = "defaults::default_s3_credentials", rename = "s3Credentials")]
    pub s3_credentials: Option<S3Credentials>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, JsonSchema)]
pub struct Restore {
    #[serde(rename = "serverName")]
    pub server_name: String,
    #[serde(rename = "recoveryTargetTime")]
    pub recovery_target_time: Option<String>,
    #[serde(default, rename = "endpointURL")]
    pub endpoint_url: Option<String>,
    #[serde(rename = "s3Credentials")]
    pub s3_credentials: Option<S3Credentials>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, ToSchema, Default)]
#[allow(non_snake_case)]
pub struct ConnectionPooler {
    #[serde(default = "defaults::default_conn_pooler_enabled")]
    pub enabled: bool,
    #[serde(default = "defaults::default_pgbouncer")]
    pub pooler: PgBouncer,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, ToSchema, Default)]
#[allow(non_snake_case)]
pub struct PgBouncer {
    #[serde(default = "defaults::default_pool_mode")]
    pub poolMode: PoolerPgbouncerPoolMode,
    // Valid parameter values can be found at https://www.pgbouncer.org/config.html
    pub parameters: Option<BTreeMap<String, String>>,
    pub resources: Option<PoolerTemplateSpecContainersResources>,
}

/// Generate the Kubernetes wrapper struct `CoreDB` from our Spec and Status struct
///
/// This provides a hook for generating the CRD yaml (in crdgen.rs)

/// This struct represents the specification for a CoreDB instance. It defines
/// various configuration options for deploying and managing the database.
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema, ToSchema)]
#[cfg_attr(test, derive(Default))]
#[kube(kind = "CoreDB", group = "coredb.io", version = "v1alpha1", namespaced)]
#[kube(status = "CoreDBStatus", shortname = "cdb")]
#[allow(non_snake_case)]
pub struct CoreDBSpec {
    /// Number of CoreDB replicas to deploy. Defaults to 1.
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

    pub extra_domains_rw: Option<Vec<String>>,

    /// List of IPv4 CIDR blocks
    #[serde(rename = "ipAllowList")]
    pub ip_allow_list: Option<Vec<String>>,

    pub stack: Option<Stack>,
    // dynamic runtime configs
    pub runtime_config: Option<Vec<PgConfig>>,
    // configuration overrides, typically defined by the user
    pub override_configs: Option<Vec<PgConfig>>,
    // Connection pooler configuration
    #[serde(default = "defaults::default_conn_pooler")]
    pub connectionPooler: ConnectionPooler,
    #[serde(rename = "appServices")]
    pub app_services: Option<Vec<AppService>>,

    // instance restore from backup
    pub restore: Option<Restore>,

    // Expose storage class to allow user to specify a custom storage class
    #[serde(rename = "storageClass")]
    pub storage_class: Option<String>,
}

impl CoreDBSpec {
    // extracts all postgres configurations
    // configs can be defined in several different places (from a stack, user override, from an extension installation, user overrides, etc)
    pub fn get_pg_configs(
        &self,
        requires_load: BTreeMap<String, String>,
    ) -> Result<Option<Vec<PgConfig>>, MergeError> {
        let stack_configs = self
            .stack
            .as_ref()
            .and_then(|s| s.postgres_config.clone())
            .unwrap_or_default();
        let mut runtime_configs = self.runtime_config.clone().unwrap_or_default();
        // TODO: configs that come with extension installation
        // e.g. let extension_configs = ...
        // these extensions could be set by the operator, or trunk + operator
        // trunk install pg_partman could come with something like `pg_partman_bgw.dbname = xxx`

        // Get list of extension names that require load
        let mut include_with_shared_preload_libraries = BTreeSet::new();
        for ext in self.extensions.iter() {
            'loc: for location in ext.locations.iter() {
                if location.enabled && requires_load.contains_key(&ext.name) {
                    if let Some(library_name) = requires_load.get(&ext.name) {
                        include_with_shared_preload_libraries.insert(library_name.clone());
                    } else {
                        // coredb name not in scope, so can't be included in log
                        error!(
                            "Extension {} requires load but no library name was found",
                            ext.name
                        );
                    }
                    break 'loc;
                }
            }
        }

        let shared_preload_from_extensions = ConfigValue::Multiple(include_with_shared_preload_libraries);
        let extension_settings_config = vec![PgConfig {
            name: "shared_preload_libraries".to_string(),
            value: shared_preload_from_extensions,
        }];

        match merge_pg_configs(
            &runtime_configs,
            &extension_settings_config,
            "shared_preload_libraries",
        )? {
            None => {}
            Some(new_shared_preload_libraries) => {
                // check by name attribute if runtime_configs already has shared_preload_libraries
                // if so replace the value. Otherwise add this PgConfig into the vector.
                let mut found = false;
                for cfg in &mut runtime_configs {
                    if cfg.name == "shared_preload_libraries" {
                        cfg.value = new_shared_preload_libraries.value.clone();
                        found = true;
                        break;
                    }
                }
                if !found {
                    runtime_configs.push(new_shared_preload_libraries);
                }
            }
        }

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

    pub fn get_pg_config_by_name(
        &self,
        config_name: &str,
        requires_load: BTreeMap<String, String>,
    ) -> Result<Option<PgConfig>, MergeError> {
        let all_configs = self.get_pg_configs(requires_load)?;
        for config in all_configs.unwrap_or_default() {
            if config.name == config_name {
                return Ok(Some(config));
            }
        }
        Ok(None)
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
    pub first_recoverability_time: Option<DateTime<Utc>>,
    pub pg_postmaster_start_time: Option<DateTime<Utc>>,
    pub last_fully_reconciled_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_core_db_spec() {
        let json_str = r#"
        {
          "image": "quay.io/tembo/tembo-pg-cnpg:15.3.0-5-cede445",
          "stack": {
            "name": "MessageQueue",
            "image": "quay.io/tembo/tembo-pg-cnpg:15.3.0-5-cede445",
            "services": null,
            "extensions": [
              {
                "name": "pgmq",
                "locations": [
                  {
                    "schema": null,
                    "enabled": true,
                    "version": "0.10.2",
                    "database": "postgres"
                  }
                ],
                "description": null
              },
              {
                "name": "pg_partman",
                "locations": [
                  {
                    "schema": null,
                    "enabled": true,
                    "version": "4.7.3",
                    "database": "postgres"
                  }
                ],
                "description": null
              }
            ],
            "description": "A Tembo Postgres Stack optimized for Message Queue workloads.",
            "stack_version": "0.2.0",
            "infrastructure": {
              "cpu": "1",
              "memory": "1Gi",
              "region": "us-east-1",
              "provider": "aws",
              "storage_size": "10Gi",
              "instance_type": "GeneralPurpose",
              "storage_class": "gp3"
            },
            "trunk_installs": [
              {
                "name": "pgmq",
                "version": "0.10.2"
              },
              {
                "name": "pg_partman",
                "version": "4.7.3"
              }
            ],
            "postgres_config": [
              {
                "name": "shared_preload_libraries",
                "value": "pg_stat_statements,pg_partman_bgw"
              },
              {
                "name": "pg_partman_bgw.dbname",
                "value": "postgres"
              },
              {
                "name": "pg_partman_bgw.interval",
                "value": "60"
              },
              {
                "name": "pg_partman_bgw.role",
                "value": "postgres"
              },
              {
                "name": "random_page_cost",
                "value": "1.1"
              },
              {
                "name": "autovacuum_vacuum_cost_limit",
                "value": "-1"
              },
              {
                "name": "autovacuum_vacuum_scale_factor",
                "value": "0.05"
              },
              {
                "name": "autovacuum_vacuum_insert_scale_factor",
                "value": "0.05"
              },
              {
                "name": "autovacuum_analyze_scale_factor",
                "value": "0.05"
              },
              {
                "name": "checkpoint_timeout",
                "value": "10min"
              },
              {
                "name": "pg_stat_statements.track",
                "value": "all"
              }
            ],
            "postgres_metrics": {
              "pgmq": {
                "query": "select queue_name, queue_length, oldest_msg_age_sec, newest_msg_age_sec, total_messages from public.pgmq_metrics_all()",
                "master": true,
                "metrics": [
                  {
                    "queue_name": {
                      "usage": "LABEL",
                      "description": "Name of the queue"
                    }
                  },
                  {
                    "queue_length": {
                      "usage": "GAUGE",
                      "description": "Number of messages in the queue"
                    }
                  },
                  {
                    "oldest_msg_age_sec": {
                      "usage": "GAUGE",
                      "description": "Age of the oldest message in the queue, in seconds."
                    }
                  },
                  {
                    "newest_msg_age_sec": {
                      "usage": "GAUGE",
                      "description": "Age of the newest message in the queue, in seconds."
                    }
                  },
                  {
                    "total_messages": {
                      "usage": "GAUGE",
                      "description": "Total number of messages that have passed into the queue."
                    }
                  }
                ]
              }
            },
            "postgres_config_engine": "standard"
          },
          "metrics": {
            "image": "quay.io/prometheuscommunity/postgres-exporter:v0.12.0",
            "enabled": true,
            "queries": {
              "pgmq": {
                "query": "select queue_name, queue_length, oldest_msg_age_sec, newest_msg_age_sec, total_messages from public.pgmq_metrics_all()",
                "master": true,
                "metrics": [
                  {
                    "queue_name": {
                      "usage": "LABEL",
                      "description": "Name of the queue"
                    }
                  },
                  {
                    "queue_length": {
                      "usage": "GAUGE",
                      "description": "Number of messages in the queue"
                    }
                  },
                  {
                    "oldest_msg_age_sec": {
                      "usage": "GAUGE",
                      "description": "Age of the oldest message in the queue, in seconds."
                    }
                  },
                  {
                    "newest_msg_age_sec": {
                      "usage": "GAUGE",
                      "description": "Age of the newest message in the queue, in seconds."
                    }
                  },
                  {
                    "total_messages": {
                      "usage": "GAUGE",
                      "description": "Total number of messages that have passed into the queue."
                    }
                  }
                ]
              }
            }
          },
          "storage": "10Gi",
          "resources": {
            "limits": {
              "cpu": "1",
              "memory": "1Gi"
            }
          },
          "extensions": [
            {
              "name": "pgmq",
              "locations": [
                {
                  "schema": null,
                  "enabled": true,
                  "version": "0.10.2",
                  "database": "postgres"
                }
              ],
              "description": null
            },
            {
              "name": "pg_partman",
              "locations": [
                {
                  "schema": null,
                  "enabled": true,
                  "version": "4.7.3",
                  "database": "postgres"
                }
              ],
              "description": null
            }
          ],
          "runtime_config": [
            {
              "name": "shared_buffers",
              "value": "256MB"
            },
            {
              "name": "max_connections",
              "value": "107"
            },
            {
              "name": "work_mem",
              "value": "5MB"
            },
            {
              "name": "bgwriter_delay",
              "value": "200ms"
            },
            {
              "name": "effective_cache_size",
              "value": "716MB"
            },
            {
              "name": "maintenance_work_mem",
              "value": "64MB"
            },
            {
              "name": "max_wal_size",
              "value": "2GB"
            }
          ],
          "trunk_installs": [
            {
              "name": "pgmq",
              "version": "0.10.2"
            },
            {
              "name": "pg_partman",
              "version": "4.7.3"
            }
          ],
          "postgresExporterEnabled": true
        }
        "#;

        let _deserialized_spec: CoreDBSpec = serde_json::from_str(json_str).unwrap();
    }
}
