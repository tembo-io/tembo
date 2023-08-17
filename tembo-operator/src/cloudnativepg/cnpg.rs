use crate::{
    apis::{coredb_types::CoreDB, postgres_parameters::MergeError},
    cloudnativepg::{
        clusters::{
            Cluster, ClusterAffinity, ClusterBackup, ClusterBackupBarmanObjectStore,
            ClusterBackupBarmanObjectStoreData, ClusterBackupBarmanObjectStoreDataCompression,
            ClusterBackupBarmanObjectStoreDataEncryption, ClusterBackupBarmanObjectStoreS3Credentials,
            ClusterBackupBarmanObjectStoreWal, ClusterBackupBarmanObjectStoreWalCompression,
            ClusterBackupBarmanObjectStoreWalEncryption, ClusterBootstrap, ClusterBootstrapInitdb,
            ClusterExternalClusters, ClusterExternalClustersPassword, ClusterLogLevel, ClusterMonitoring,
            ClusterMonitoringCustomQueriesConfigMap, ClusterNodeMaintenanceWindow, ClusterPostgresql,
            ClusterPostgresqlSyncReplicaElectionConstraint, ClusterPrimaryUpdateMethod,
            ClusterPrimaryUpdateStrategy, ClusterResources, ClusterServiceAccountTemplate,
            ClusterServiceAccountTemplateMetadata, ClusterSpec, ClusterStorage, ClusterSuperuserSecret,
        },
        scheduledbackups::{
            ScheduledBackup, ScheduledBackupBackupOwnerReference, ScheduledBackupCluster, ScheduledBackupSpec,
        },
    },
    config::Config,
    defaults::{default_image, default_llm_image},
    Context,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::{
    api::{Patch, PatchParams},
    runtime::controller::Action,
    Api, Resource, ResourceExt,
};
use std::{collections::BTreeMap, sync::Arc};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

pub struct PostgresConfig {
    pub postgres_parameters: Option<BTreeMap<String, String>>,
    pub shared_preload_libraries: Option<Vec<String>>,
}

pub fn cnpg_backup_configuration(
    cdb: &CoreDB,
    cfg: &Config,
) -> (Option<ClusterBackup>, Option<ClusterServiceAccountTemplate>) {
    // Check to make sure that backups are enabled, and return None if it is disabled.
    if !cfg.enable_backup {
        warn!("Backups are disabled");
        (None, None)
    } else {
        debug!("Backups are enabled, configuring...");

        let backup_path = cdb.spec.backup.destinationPath.clone();
        if backup_path.is_none() {
            warn!("Backups are disabled because we don't have an S3 backup path");
            return (None, None);
        }
        let service_account_metadata = cdb.spec.serviceAccountTemplate.metadata.clone();
        if service_account_metadata.is_none() {
            warn!("Backups are disabled because we don't have a service account template");
            return (None, None);
        }
        let service_account_annotations = service_account_metadata
            .expect("Expected service account template metadata")
            .annotations;
        if service_account_annotations.is_none() {
            warn!("Backups are disabled because we don't have a service account template with annotations");
            return (None, None);
        }
        let service_account_annotations =
            service_account_annotations.expect("Expected service account template annotations");
        let service_account_role_arn = service_account_annotations.get("eks.amazonaws.com/role-arn");
        if service_account_role_arn.is_none() {
            warn!(
                "Backups are disabled because we don't have a service account template with an EKS role ARN"
            );
            return (None, None);
        }
        let role_arn = service_account_role_arn
            .expect("Expected service account template annotations to contain an EKS role ARN")
            .clone();

        let retention_days = match &cdb.spec.backup.retentionPolicy {
            None => "30d".to_string(),
            Some(retention_policy) => {
                match retention_policy.parse::<i32>() {
                    Ok(days) => {
                        format!("{}d", days)
                    }
                    Err(_) => {
                        warn!("Invalid retention policy because could not convert to i32, using default of 30 days");
                        "30d".to_string()
                    }
                }
            }
        };

        let cluster_backup = Some(ClusterBackup {
            barman_object_store: Some(ClusterBackupBarmanObjectStore {
                data: Some(ClusterBackupBarmanObjectStoreData {
                    compression: Some(ClusterBackupBarmanObjectStoreDataCompression::Bzip2),
                    encryption: Some(ClusterBackupBarmanObjectStoreDataEncryption::Aes256),
                    immediate_checkpoint: Some(true),
                    ..ClusterBackupBarmanObjectStoreData::default()
                }),
                destination_path: backup_path.expect("Expected to find S3 path"),
                s3_credentials: Some(ClusterBackupBarmanObjectStoreS3Credentials {
                    inherit_from_iam_role: Some(true),
                    ..ClusterBackupBarmanObjectStoreS3Credentials::default()
                }),
                wal: Some(ClusterBackupBarmanObjectStoreWal {
                    compression: Some(ClusterBackupBarmanObjectStoreWalCompression::Bzip2),
                    encryption: Some(ClusterBackupBarmanObjectStoreWalEncryption::Aes256),
                    max_parallel: Some(5),
                }),
                ..ClusterBackupBarmanObjectStore::default()
            }),
            retention_policy: Some(retention_days),
            ..ClusterBackup::default()
        });

        let service_account_template = Some(ClusterServiceAccountTemplate {
            metadata: ClusterServiceAccountTemplateMetadata {
                annotations: Some(BTreeMap::from([(
                    "eks.amazonaws.com/role-arn".to_string(),
                    role_arn,
                )])),
                ..ClusterServiceAccountTemplateMetadata::default()
            },
        });

        (cluster_backup, service_account_template)
    }
}

pub fn cnpg_cluster_bootstrap_from_cdb(
    cdb: &CoreDB,
) -> (
    Option<ClusterBootstrap>,
    Option<Vec<ClusterExternalClusters>>,
    Option<ClusterSuperuserSecret>,
) {
    // todo: Add logic if restore is needed
    let cluster_bootstrap = ClusterBootstrap {
        initdb: Some(ClusterBootstrapInitdb {
            ..ClusterBootstrapInitdb::default()
        }),
        ..ClusterBootstrap::default()
    };
    let cluster_name = cdb.name_any();

    let mut coredb_connection_parameters = BTreeMap::new();
    coredb_connection_parameters.insert("user".to_string(), "postgres".to_string());
    // The CoreDB operator rw service name is the CoreDB cluster name
    coredb_connection_parameters.insert("host".to_string(), cluster_name.clone());

    let superuser_secret_name = format!("{}-connection", cluster_name);

    let coredb_cluster = ClusterExternalClusters {
        name: "coredb".to_string(),
        connection_parameters: Some(coredb_connection_parameters),
        password: Some(ClusterExternalClustersPassword {
            // The CoreDB operator connection secret is named as the cluster
            // name, suffixed by -connection
            name: Some(superuser_secret_name.clone()),
            key: "password".to_string(),
            ..ClusterExternalClustersPassword::default()
        }),
        ..ClusterExternalClusters::default()
    };

    let superuser_secret = ClusterSuperuserSecret {
        name: superuser_secret_name,
    };

    (
        Some(cluster_bootstrap),
        Some(vec![coredb_cluster]),
        Some(superuser_secret),
    )
}

// Get PGConfig from CoreDB and convert it to a postgres_parameters and shared_preload_libraries
fn cnpg_postgres_config(cdb: &CoreDB) -> Result<PostgresConfig, MergeError> {
    match cdb.spec.get_pg_configs() {
        Ok(Some(pg_configs)) => {
            let mut postgres_parameters: BTreeMap<String, String> = BTreeMap::new();
            let mut shared_preload_libraries: Vec<String> = Vec::new();

            for pg_config in pg_configs {
                match &pg_config.name[..] {
                    "shared_preload_libraries" => {
                        shared_preload_libraries.push(pg_config.value.to_string());
                    }
                    _ => {
                        postgres_parameters.insert(pg_config.name.clone(), pg_config.value.to_string());
                    }
                }
            }

            let params = if postgres_parameters.is_empty() {
                None
            } else {
                Some(postgres_parameters)
            };

            let libs = if shared_preload_libraries.is_empty() {
                None
            } else {
                Some(shared_preload_libraries)
            };

            Ok(PostgresConfig {
                postgres_parameters: params,
                shared_preload_libraries: libs,
            })
        }
        Ok(None) => {
            // Return None, None when no pg_config is set
            Ok(PostgresConfig {
                postgres_parameters: None,
                shared_preload_libraries: None,
            })
        }
        Err(e) => Err(e),
    }
}

fn cnpg_cluster_storage(cdb: &CoreDB) -> Option<ClusterStorage> {
    let storage = cdb.spec.storage.clone().0;
    Some(ClusterStorage {
        resize_in_use_volumes: Some(true),
        size: Some(storage),
        // TODO: pass storage class from cdb
        // storage_class: Some("gp3-enc".to_string()),
        storage_class: None,
        ..ClusterStorage::default()
    })
}

pub fn cnpg_cluster_from_cdb(cdb: &CoreDB) -> Cluster {
    let cfg = Config::default();
    let name = cdb.name_any();
    let namespace = cdb.namespace().unwrap();
    let owner_reference = cdb.controller_owner_ref(&()).unwrap();

    let mut annotations = BTreeMap::new();
    annotations.insert("tembo-pod-init.tembo.io/inject".to_string(), "true".to_string());

    let (bootstrap, external_clusters, superuser_secret) = cnpg_cluster_bootstrap_from_cdb(cdb);

    let (backup, service_account_template) = cnpg_backup_configuration(cdb, &cfg);

    let storage = cnpg_cluster_storage(cdb);

    let PostgresConfig {
        postgres_parameters,
        shared_preload_libraries,
    } = match cnpg_postgres_config(cdb) {
        Ok(config) => config,
        Err(e) => {
            error!("Error generating postgres parameters: {}", e);
            PostgresConfig {
                postgres_parameters: None,
                shared_preload_libraries: None,
            }
        }
    };

    // set the container image
    // Check if the cdb.spec.image is set, if not then figure out which image to use.
    let image = if cdb.spec.image.is_empty() {
        match cdb.spec.stack.as_ref().map(|s| s.name.to_lowercase()) {
            Some(ref name) if name == "machinelearning" => default_llm_image(),
            _ => default_image(),
        }
    } else {
        cdb.spec.image.clone()
    };

    Cluster {
        metadata: ObjectMeta {
            name: Some(name),
            namespace: Some(namespace),
            annotations: Some(annotations),
            owner_references: Some(vec![owner_reference]),
            ..ObjectMeta::default()
        },
        spec: ClusterSpec {
            affinity: Some(ClusterAffinity {
                pod_anti_affinity_type: Some("preferred".to_string()),
                topology_key: Some("topology.kubernetes.io/zone".to_string()),
                ..ClusterAffinity::default()
            }),
            backup,
            service_account_template,
            bootstrap,
            superuser_secret,
            external_clusters,
            enable_superuser_access: Some(true),
            failover_delay: Some(0),
            image_name: Some(image),
            instances: 1,
            log_level: Some(ClusterLogLevel::Info),
            max_sync_replicas: Some(0),
            min_sync_replicas: Some(0),
            monitoring: Some(ClusterMonitoring {
                custom_queries_config_map: Some(vec![ClusterMonitoringCustomQueriesConfigMap {
                    key: "queries".to_string(),
                    name: "cnpg-default-monitoring".to_string(),
                }]),
                disable_default_queries: Some(false),
                enable_pod_monitor: Some(true),
                ..ClusterMonitoring::default()
            }),
            postgres_gid: Some(26),
            postgres_uid: Some(26),
            postgresql: Some(ClusterPostgresql {
                ldap: None,
                parameters: postgres_parameters,
                sync_replica_election_constraint: Some(ClusterPostgresqlSyncReplicaElectionConstraint {
                    enabled: false,
                    ..ClusterPostgresqlSyncReplicaElectionConstraint::default()
                }),
                shared_preload_libraries,
                pg_hba: None,
                ..ClusterPostgresql::default()
            }),
            primary_update_method: Some(ClusterPrimaryUpdateMethod::Restart),
            primary_update_strategy: Some(ClusterPrimaryUpdateStrategy::Unsupervised),
            resources: Some(ClusterResources {
                claims: None,
                limits: cdb.spec.resources.clone().limits,
                requests: cdb.spec.resources.clone().requests,
            }),
            // The time in seconds that is allowed for a PostgreSQL instance to successfully start up
            start_delay: Some(30),
            // The time in seconds that is allowed for a PostgreSQL instance to gracefully shutdown
            stop_delay: Some(30),
            storage,
            // The time in seconds that is allowed for a primary PostgreSQL instance
            // to gracefully shutdown during a switchover
            switchover_delay: Some(60),
            // Set this to match when the cluster consolidation happens
            node_maintenance_window: Some(ClusterNodeMaintenanceWindow {
                // TODO TEM-1407: Make this configurable and aligned with cluster scale down
                // default to in_progress: true - otherwise single-instance CNPG clusters
                // prevent cluster scale down.
                in_progress: true,
                ..ClusterNodeMaintenanceWindow::default()
            }),
            ..ClusterSpec::default()
        },
        status: None,
    }
}

pub async fn reconcile_cnpg(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Action> {
    debug!("Generating CNPG spec");
    let mut cluster = cnpg_cluster_from_cdb(cdb);

    debug!("Getting namespace of cluster");
    let namespace = cluster
        .metadata
        .namespace
        .clone()
        .expect("CNPG Cluster should always have a namespace");
    debug!("Getting name of cluster");
    let name = cluster
        .metadata
        .name
        .clone()
        .expect("CNPG Cluster should always have a name");
    let cluster_api: Api<Cluster> = Api::namespaced(ctx.client.clone(), namespace.as_str());

    let mut _restart_required = false;

    match cluster
        .spec
        .postgresql
        .clone()
        .expect("We always set the postgresql spec")
        .shared_preload_libraries
    {
        None => {
            debug!("We are not setting any shared_preload_libraries");
        }
        Some(new_libs) => {
            debug!("We are setting shared_preload_libraries, so we have to check if the files are already installed");
            match cluster_api.get(&name).await {
                Ok(current_cluster) => {
                    let current_shared_preload_libraries = match current_cluster
                        .spec
                        .postgresql
                        .clone()
                        .expect("We always set postgresql cluster spec")
                        .shared_preload_libraries
                    {
                        None => {
                            let current_libs: Vec<String> = vec![];
                            current_libs
                        }
                        Some(current_libs) => current_libs,
                    };
                    // Check if current_shared_preload_libraries and new_libs are the same
                    if current_shared_preload_libraries != new_libs {
                        let mut libs_that_are_installed: Vec<String> = vec![];
                        // If we can't find the existing primary pod, returns a requeue
                        let primary_pod_cnpg = cdb.primary_pod_cnpg(ctx.client.clone()).await?;
                        // Check if the file is already installed
                        let command = vec![
                            "/bin/sh".to_string(),
                            "-c".to_string(),
                            "ls $(pg_config --pkglibdir)".to_string(),
                        ];
                        let result = cdb
                            .exec(primary_pod_cnpg.name_any(), ctx.client.clone(), &command)
                            .await
                            .map_err(|e| {
                                error!("Error checking for presence of extension files: {:?}", e);
                                Action::requeue(Duration::from_secs(30))
                            })?;
                        let available_libs = match result.stdout {
                            None => {
                                error!("Error checking for presence of extension files");
                                return Err(Action::requeue(Duration::from_secs(30)));
                            }
                            Some(output) => {
                                output.split('\n').map(|s| s.to_string()).collect::<Vec<String>>()
                            }
                        };
                        for libs in new_libs {
                            let split_libs = libs.split(',').map(|s| s.to_string()).collect::<Vec<String>>();
                            for new_lib in split_libs {
                                if available_libs.contains(&format!("{}.so", new_lib)) {
                                    info!("Changing shared_preload_libraries on {}, found {} is installed, so including it", &name, &new_lib);
                                    libs_that_are_installed.push(new_lib.clone());
                                    if !current_shared_preload_libraries.contains(&new_lib) {
                                        _restart_required = true;
                                    }
                                } else {
                                    info!("Changing shared_preload_libraries on {}, found {} is NOT installed, so dropping it", &name, &new_lib);
                                }
                            }
                        }
                        if let Some(postgresql) = cluster.spec.postgresql.as_mut() {
                            postgresql.shared_preload_libraries = Some(libs_that_are_installed);
                        }
                    }
                }
                Err(_) => {
                    // Here, we should drop all shared_preload_libraries
                    if let Some(postgresql) = cluster.spec.postgresql.as_mut() {
                        info!(
                            "We are dropping all shared_preload_libraries for initial creation of Cluster {}",
                            &name
                        );
                        postgresql.shared_preload_libraries = None;
                    }
                }
            }
        }
    }

    debug!("Patching cluster");
    let ps = PatchParams::apply("cntrlr");
    let _o = cluster_api
        .patch(&name, &ps, &Patch::Apply(&cluster))
        .await
        .map_err(|e| {
            error!("Error patching cluster: {}", e);
            Action::requeue(Duration::from_secs(300))
        })?;
    debug!("Applied");
    // If restart is required, then we should trigger the restart above
    Ok(())
}

fn schedule_expression_from_cdb(cdb: &CoreDB) -> String {
    // Default to daily at midnight
    let default = "0 0 0 * * *".to_string();
    match &cdb.spec.backup.schedule {
        None => default,
        Some(expression) => {
            let mut terms = expression.split(' ').collect::<Vec<&str>>();
            if terms.len() == 5 {
                // pre-pend "0" to the vector
                let mut new_terms = vec!["0"];
                new_terms.extend(terms);
                terms = new_terms.clone();
            }
            if terms.len() != 6 {
                warn!("Invalid schedule expression, expected five or six terms. Setting as default. Found expression: '{}'", expression);
                return default;
            }
            // check that all terms are either parsable as int32 or "*"
            for term in &terms {
                if *term != "*" {
                    match term.parse::<i32>() {
                        Ok(_) => {}
                        Err(_) => {
                            warn!("Invalid schedule expression, only integers and '*' are accepted, setting as default. Found: {}", expression);
                            return default;
                        }
                    }
                }
            }
            terms.join(" ")
        }
    }
}

// Generate a ScheduledBackup
fn cnpg_scheduled_backup(cdb: &CoreDB) -> ScheduledBackup {
    let name = cdb.name_any();
    let namespace = cdb.namespace().unwrap();

    ScheduledBackup {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            namespace: Some(namespace),
            ..ObjectMeta::default()
        },
        spec: ScheduledBackupSpec {
            backup_owner_reference: Some(ScheduledBackupBackupOwnerReference::Cluster),
            cluster: Some(ScheduledBackupCluster { name }),
            immediate: Some(true),
            schedule: schedule_expression_from_cdb(cdb),
            suspend: Some(false),
            ..ScheduledBackupSpec::default()
        },
        status: None,
    }
}

// Reconcile a SheduledBackup
pub async fn reconcile_cnpg_scheduled_backup(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Action> {
    let scheduledbackup = cnpg_scheduled_backup(cdb);
    let client = ctx.client.clone();
    let name = scheduledbackup
        .metadata
        .name
        .clone()
        .expect("ScheduledBackup should always have a name");
    let namespace = scheduledbackup
        .metadata
        .namespace
        .clone()
        .expect("ScheduledBackup should always have a namespace");
    let backup_api: Api<ScheduledBackup> = Api::namespaced(client.clone(), namespace.as_str());

    debug!("Patching ScheduledBackup");
    let ps = PatchParams::apply("cntrlr");
    let _o = backup_api
        .patch(&name, &ps, &Patch::Apply(&scheduledbackup))
        .await
        .map_err(|e| {
            error!("Error patching ScheduledBackup: {}", e);
            Action::requeue(Duration::from_secs(300))
        })?;
    debug!("Applied ScheduledBackup");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_cluster() {
        let json_str = r#"
        {
          "apiVersion": "postgresql.cnpg.io/v1",
          "kind": "Cluster",
          "metadata": {
            "annotations": {
              "tembo-pod-init.tembo.io/inject": "true"
            },
            "creationTimestamp": "2023-07-03T18:15:32Z",
            "generation": 1,
            "managedFields": [
              {
                "apiVersion": "postgresql.cnpg.io/v1",
                "fieldsType": "FieldsV1",
                "fieldsV1": {
                  "f:metadata": {
                    "f:annotations": {
                      "f:tembo-pod-init.tembo.io/inject": {}
                    },
                    "f:ownerReferences": {
                      "k:{\"uid\":\"d8efe1ff-b09a-43ca-a568-def96235e754\"}": {}
                    }
                  },
                  "f:spec": {
                    "f:affinity": {
                      "f:podAntiAffinityType": {},
                      "f:topologyKey": {}
                    },
                    "f:bootstrap": {
                      "f:pg_basebackup": {
                        "f:source": {}
                      }
                    },
                    "f:enableSuperuserAccess": {},
                    "f:externalClusters": {},
                    "f:failoverDelay": {},
                    "f:imageName": {},
                    "f:instances": {},
                    "f:logLevel": {},
                    "f:maxSyncReplicas": {},
                    "f:minSyncReplicas": {},
                    "f:monitoring": {
                      "f:customQueriesConfigMap": {},
                      "f:disableDefaultQueries": {},
                      "f:enablePodMonitor": {}
                    },
                    "f:postgresGID": {},
                    "f:postgresUID": {},
                    "f:postgresql": {
                      "f:parameters": {
                        "f:archive_mode": {},
                        "f:archive_timeout": {},
                        "f:dynamic_shared_memory_type": {},
                        "f:log_destination": {},
                        "f:log_directory": {},
                        "f:log_filename": {},
                        "f:log_rotation_age": {},
                        "f:log_rotation_size": {},
                        "f:log_truncate_on_rotation": {},
                        "f:logging_collector": {},
                        "f:max_parallel_workers": {},
                        "f:max_replication_slots": {},
                        "f:max_worker_processes": {},
                        "f:shared_memory_type": {},
                        "f:wal_keep_size": {},
                        "f:wal_receiver_timeout": {},
                        "f:wal_sender_timeout": {}
                      },
                      "f:syncReplicaElectionConstraint": {
                        "f:enabled": {}
                      }
                    },
                    "f:primaryUpdateMethod": {},
                    "f:primaryUpdateStrategy": {},
                    "f:startDelay": {},
                    "f:stopDelay": {},
                    "f:storage": {
                      "f:resizeInUseVolumes": {},
                      "f:size": {}
                    },
                    "f:superuserSecret": {
                      "f:name": {}
                    },
                    "f:switchoverDelay": {}
                  }
                },
                "manager": "cntrlr",
                "operation": "Apply",
                "time": "2023-07-03T18:15:32Z"
              },
              {
                "apiVersion": "postgresql.cnpg.io/v1",
                "fieldsType": "FieldsV1",
                "fieldsV1": {
                  "f:status": {
                    ".": {},
                    "f:certificates": {
                      ".": {},
                      "f:clientCASecret": {},
                      "f:expirations": {
                        ".": {},
                        "f:test-coredb-ca": {},
                        "f:test-coredb-replication": {},
                        "f:test-coredb-server": {}
                      },
                      "f:replicationTLSSecret": {},
                      "f:serverAltDNSNames": {},
                      "f:serverCASecret": {},
                      "f:serverTLSSecret": {}
                    },
                    "f:cloudNativePGCommitHash": {},
                    "f:cloudNativePGOperatorHash": {},
                    "f:conditions": {},
                    "f:configMapResourceVersion": {
                      ".": {},
                      "f:metrics": {
                        ".": {},
                        "f:cnpg-default-monitoring": {}
                      }
                    },
                    "f:healthyPVC": {},
                    "f:instanceNames": {},
                    "f:instances": {},
                    "f:instancesStatus": {
                      ".": {},
                      "f:failed": {}
                    },
                    "f:jobCount": {},
                    "f:latestGeneratedNode": {},
                    "f:managedRolesStatus": {},
                    "f:phase": {},
                    "f:phaseReason": {},
                    "f:poolerIntegrations": {
                      ".": {},
                      "f:pgBouncerIntegration": {}
                    },
                    "f:pvcCount": {},
                    "f:readService": {},
                    "f:secretsResourceVersion": {
                      ".": {},
                      "f:clientCaSecretVersion": {},
                      "f:replicationSecretVersion": {},
                      "f:serverCaSecretVersion": {},
                      "f:serverSecretVersion": {},
                      "f:superuserSecretVersion": {}
                    },
                    "f:targetPrimary": {},
                    "f:targetPrimaryTimestamp": {},
                    "f:topology": {
                      ".": {},
                      "f:instances": {
                        ".": {},
                        "f:test-coredb-1": {}
                      },
                      "f:successfullyExtracted": {}
                    },
                    "f:writeService": {}
                  }
                },
                "manager": "Go-http-client",
                "operation": "Update",
                "subresource": "status",
                "time": "2023-07-03T18:16:49Z"
              }
            ],
            "name": "test-coredb",
            "namespace": "default",
            "ownerReferences": [
              {
                "apiVersion": "coredb.io/v1alpha1",
                "controller": true,
                "kind": "CoreDB",
                "name": "test-coredb",
                "uid": "d8efe1ff-b09a-43ca-a568-def96235e754"
              }
            ],
            "resourceVersion": "7675",
            "uid": "7bfae8f4-bc86-481b-8f7c-7a7a659da265"
          },
          "spec": {
            "affinity": {
              "podAntiAffinityType": "preferred",
              "topologyKey": "topology.kubernetes.io/zone"
            },
            "bootstrap": {
              "pg_basebackup": {
                "database": "",
                "owner": "",
                "source": "coredb"
              }
            },
            "enableSuperuserAccess": true,
            "externalClusters": [
              {
                "connectionParameters": {
                  "host": "test-coredb",
                  "user": "postgres"
                },
                "name": "coredb",
                "password": {
                  "key": "password",
                  "name": "test-coredb-connection"
                }
              }
            ],
            "failoverDelay": 0,
            "imageName": "quay.io/tembo/tembo-pg-cnpg:15.3.0-1-3953e4e",
            "instances": 1,
            "logLevel": "info",
            "maxSyncReplicas": 0,
            "minSyncReplicas": 0,
            "monitoring": {
              "customQueriesConfigMap": [
                {
                  "key": "queries",
                  "name": "cnpg-default-monitoring"
                }
              ],
              "disableDefaultQueries": false,
              "enablePodMonitor": true
            },
            "postgresGID": 26,
            "postgresUID": 26,
            "postgresql": {
              "parameters": {
                "archive_mode": "on",
                "archive_timeout": "5min",
                "dynamic_shared_memory_type": "posix",
                "log_destination": "csvlog",
                "log_directory": "/controller/log",
                "log_filename": "postgres",
                "log_rotation_age": "0",
                "log_rotation_size": "0",
                "log_truncate_on_rotation": "false",
                "logging_collector": "on",
                "max_parallel_workers": "32",
                "max_replication_slots": "32",
                "max_worker_processes": "32",
                "shared_memory_type": "mmap",
                "shared_preload_libraries": "",
                "wal_keep_size": "512MB",
                "wal_receiver_timeout": "5s",
                "wal_sender_timeout": "5s"
              },
              "syncReplicaElectionConstraint": {
                "enabled": false
              }
            },
            "primaryUpdateMethod": "restart",
            "primaryUpdateStrategy": "unsupervised",
            "resources": {},
            "startDelay": 30,
            "stopDelay": 30,
            "storage": {
              "resizeInUseVolumes": true,
              "size": "1Gi"
            },
            "superuserSecret": {
              "name": "test-coredb-connection"
            },
            "switchoverDelay": 60
          },
          "status": {
            "certificates": {
              "clientCASecret": "test-coredb-ca",
              "expirations": {
                "test-coredb-ca": "2023-10-01 18:10:32 +0000 UTC",
                "test-coredb-replication": "2023-10-01 18:10:32 +0000 UTC",
                "test-coredb-server": "2023-10-01 18:10:32 +0000 UTC"
              },
              "replicationTLSSecret": "test-coredb-replication",
              "serverAltDNSNames": [
                "test-coredb-rw",
                "test-coredb-rw.default",
                "test-coredb-rw.default.svc",
                "test-coredb-r",
                "test-coredb-r.default",
                "test-coredb-r.default.svc",
                "test-coredb-ro",
                "test-coredb-ro.default",
                "test-coredb-ro.default.svc"
              ],
              "serverCASecret": "test-coredb-ca",
              "serverTLSSecret": "test-coredb-server"
            },
            "cloudNativePGCommitHash": "9bf74c9e",
            "cloudNativePGOperatorHash": "5d5f339b30506db0996606d61237dcf639c1e0d3009c0399e87e99cc7bc2caf0",
            "conditions": [
              {
                "lastTransitionTime": "2023-07-03T18:15:32Z",
                "message": "Cluster Is Not Ready",
                "reason": "ClusterIsNotReady",
                "status": "False",
                "type": "Ready"
              }
            ],
            "configMapResourceVersion": {
              "metrics": {
                "cnpg-default-monitoring": "7435"
              }
            },
            "healthyPVC": [
              "test-coredb-1"
            ],
            "instanceNames": [
              "test-coredb-1"
            ],
            "instances": 1,
            "instancesStatus": {
              "failed": [
                "test-coredb-1"
              ]
            },
            "jobCount": 1,
            "latestGeneratedNode": 1,
            "managedRolesStatus": {},
            "phase": "Setting up primary",
            "phaseReason": "Creating primary instance test-coredb-1",
            "poolerIntegrations": {
              "pgBouncerIntegration": {}
            },
            "pvcCount": 1,
            "readService": "test-coredb-r",
            "secretsResourceVersion": {
              "clientCaSecretVersion": "7409",
              "replicationSecretVersion": "7411",
              "serverCaSecretVersion": "7409",
              "serverSecretVersion": "7410",
              "superuserSecretVersion": "7107"
            },
            "targetPrimary": "test-coredb-1",
            "targetPrimaryTimestamp": "2023-07-03T18:15:32.464538Z",
            "topology": {
              "instances": {
                "test-coredb-1": {}
              },
              "successfullyExtracted": true
            },
            "writeService": "test-coredb-rw"
          }
        }

        "#;

        let _result: Cluster = serde_json::from_str(json_str).expect("Should be able to deserialize");
    }

    use serde_yaml::from_str;

    #[test]
    fn test_cnpg_scheduled_backup() {
        // Arrange
        let cdb_yaml = r#"
        apiVersion: coredb.io/v1alpha1
        kind: CoreDB
        metadata:
          name: test
          namespace: default
        spec:
          backup:
            destinationPath: s3://aws-s3-bucket/tembo/backup
            encryption: AES256
            retentionPolicy: "45"
            schedule: 55 7 * * *
          image: quay.io/tembo/tembo-pg-cnpg:15.3.0-5-48d489e 
          port: 5432
          postgresExporterEnabled: true
          postgresExporterImage: quay.io/prometheuscommunity/postgres-exporter:v0.12.1
          replicas: 1
          resources:
            limits:
              cpu: "1"
              memory: 0.5Gi
          serviceAccountTemplate:
            metadata:
              annotations:
                eks.amazonaws.com/role-arn: arn:aws:iam::012345678901:role/aws-iam-role-iam
          sharedirStorage: 1Gi
          stop: false
          storage: 1Gi
          uid: 999
        "#;
        let cdb: CoreDB = from_str(cdb_yaml).unwrap();
        let cfg = Config::default();

        let scheduled_backup: ScheduledBackup = cnpg_scheduled_backup(&cdb);
        let (backup, service_account_template) = cnpg_backup_configuration(&cdb, &cfg);

        // Assert to make sure that backup schedule is set
        assert_eq!(scheduled_backup.spec.schedule, "0 55 7 * * *".to_string());
        assert_eq!(
            backup.clone().unwrap().retention_policy.unwrap(),
            "45d".to_string()
        );

        // Assert to make sure that backup destination path is set
        assert_eq!(
            backup.unwrap().barman_object_store.unwrap().destination_path,
            "s3://aws-s3-bucket/tembo/backup".to_string()
        );

        // Assert to make sure that service account template is set
        assert_eq!(
            service_account_template
                .unwrap()
                .metadata
                .annotations
                .unwrap()
                .get("eks.amazonaws.com/role-arn")
                .unwrap(),
            "arn:aws:iam::012345678901:role/aws-iam-role-iam"
        );
    }
}
