use crate::{
    apis::coredb_types::CoreDB,
    cloudnativepg::{
        backups::{
            Backup, BackupCluster, BackupMethod, BackupOnlineConfiguration, BackupSpec,
            BackupTarget,
        },
        clusters::Cluster,
    },
    Context,
};
use chrono::{DateTime, Utc};
use k8s_openapi::{
    apimachinery::pkg::apis::meta::v1::ObjectMeta, apimachinery::pkg::apis::meta::v1::Time,
};
use kube::api::ObjectList;
use kube::{
    api::{ListParams, Patch, PatchParams},
    runtime::controller::Action,
    Api, ResourceExt,
};
use std::{collections::BTreeMap, sync::Arc};
use tokio::time::Duration;
use tracing::{debug, error, info, instrument, warn};

#[instrument(skip(cdb, ctx, cluster) fields(trace_id, instance_name = %cdb.name_any()))]
pub async fn create_backup_if_needed(
    cdb: &CoreDB,
    ctx: &Arc<Context>,
    cluster: &Cluster,
) -> Result<(), Action> {
    let name = cdb.name_any();
    let namespace = cluster.metadata.namespace.as_ref().ok_or_else(|| {
        error!(
            "Cluster namespace is empty for instance: {}.",
            name.as_str()
        );
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;

    // We need to check if the cdb.replicas is changing from 1 -> 2.  If it is, we need to take a
    // snapshot if snapshots are enabled.  We will need to wait on the snapshot to complete before
    // allowing the new Cluster configuration to be applied.
    if !replicas_increasing(cdb, cluster) {
        return Ok(());
    }

    // check if snapshots are enabled, if not return OK
    if let Some(volume_snapshot) = &cdb.spec.backup.volume_snapshot {
        if !volume_snapshot.enabled {
            return Ok(());
        }
    } else {
        return Ok(());
    }
    info!(
        "Replicas are increasing and snapshots are enabled for instance: {}",
        name.as_str()
    );

    // Check when last backup was taken from Cluster.status.lastSuccessfulBackup.
    // If longer than 60 minutes and snapshots_enabled is true then take a snapshot
    let last_backup = cluster
        .status
        .as_ref()
        .and_then(|s| s.last_successful_backup.as_ref())
        .and_then(|l| l.parse::<DateTime<Utc>>().ok());

    info!(
        "Last backup for instance {} was at: {:?}",
        name.as_str(),
        last_backup
    );

    let now = Utc::now();
    let duration = now.signed_duration_since(last_backup.unwrap_or(now));
    if duration.num_minutes() <= 60 {
        info!(
            "Last backup for instance {} was taken {:?}m ago, continue without taking a new backup",
            name.as_str(),
            duration.num_minutes(),
        );
        return Ok(());
    }

    info!(
        "Last backup for instance {} was taken {:?}m ago, taking a new backup",
        name.as_str(),
        duration.num_minutes(),
    );

    let backup_api: Api<Backup> = Api::namespaced(ctx.client.clone(), namespace);

    // List all backups for the cluster
    let lp = ListParams {
        label_selector: Some(format!("cnpg.io/cluster={}", name.as_str())),
        ..ListParams::default()
    };
    let backups = backup_api.list(&lp).await.map_err(|e| {
        error!("Error listing backups: {}", e);
        Action::requeue(Duration::from_secs(300))
    })?;

    // Filter the backups based on the spec.method field
    let filtered_backups: ObjectList<Backup> = ObjectList {
        metadata: backups.metadata,
        items: backups
            .items
            .into_iter()
            .filter(|b| b.spec.method == Some(BackupMethod::VolumeSnapshot))
            .collect(),
    };

    let currently_running_volume_snaps =
        has_currently_running_volume_snaps(&filtered_backups, Time(now));

    if currently_running_volume_snaps {
        warn!("Active backups detected, requeuing in 30 seconds");
        return Err(Action::requeue(Duration::from_secs(30)));
    }

    create_replica_snapshot(cdb, ctx.clone()).await?;

    info!(
        "Created a new backup for {}, requeuing in 30 seconds",
        name.as_str()
    );
    // Remove the `return` keyword here
    Err(Action::requeue(Duration::from_secs(30)))
}

#[instrument(skip(backups), fields(trace_id))]
fn has_currently_running_volume_snaps(backups: &ObjectList<Backup>, now: Time) -> bool {
    let mut status_match = false;
    let mut creation_time_match = false;

    for b in &backups.items {
        if !status_match {
            status_match = match b.status.as_ref() {
                Some(status) => status.phase.as_deref().map_or(true, |phase| {
                    phase.is_empty()
                        || matches!(phase, "started" | "running" | "pending" | "finalizing")
                }),
                None => true,
            };
        }

        if !creation_time_match {
            creation_time_match =
                b.metadata
                    .creation_timestamp
                    .as_ref()
                    .map_or(false, |creation_time| {
                        let duration = now.0.signed_duration_since(creation_time.0);
                        duration.num_minutes() <= 60
                    });
        }

        if status_match {
            info!(
                "Backup {} is currently in progress or has no status.",
                b.metadata.name.as_deref().unwrap_or("unknown")
            );
            return true;
        }

        if creation_time_match {
            info!(
                "Backup {} was created within the last 60 minutes.",
                b.metadata.name.as_deref().unwrap_or("unknown")
            );
            return true;
        }
    }

    info!("No currently running or recent backups found.");
    false
}

// create_replica_snapshot creates a snapshot (backup) of the current primary instance
// so we can create a new replica from it.
#[instrument(skip(cdb, ctx), fields(trace_id, instance_name = %cdb.name_any()))]
async fn create_replica_snapshot(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Action> {
    let name = cdb.name_any();
    let namespace = cdb.metadata.namespace.as_ref().ok_or_else(|| {
        error!("CoreDB namespace is empty for instance: {}.", name);
        Action::requeue(tokio::time::Duration::from_secs(299))
    })?;

    // Setup the API to Backup
    let backup_api: Api<Backup> = Api::namespaced(ctx.client.clone(), namespace);

    // Generate snapshot name.  Should be name + replica + date (replica-20240327045601) but also less than 54 characters
    let timestamp = to_compact_iso8601(Utc::now());
    let snapshot_name = generate_snapshot_name(&name, &timestamp);

    // Generate the labels for the backup object
    let labels = BTreeMap::from([
        (String::from("cnpg.io/cluster"), name.clone()),
        (
            String::from("cnpg.io/immediateBackup"),
            String::from("true"),
        ),
    ]);

    // Gererate the Backup object
    let backup = Backup {
        metadata: ObjectMeta {
            name: Some(snapshot_name.clone()),
            namespace: Some(namespace.to_string()),
            labels: Some(labels),
            ..ObjectMeta::default()
        },
        spec: BackupSpec {
            cluster: BackupCluster { name: name.clone() },
            method: Some(BackupMethod::VolumeSnapshot),
            online: Some(true),
            online_configuration: Some(BackupOnlineConfiguration {
                immediate_checkpoint: Some(true),
                ..BackupOnlineConfiguration::default()
            }),
            target: Some(BackupTarget::Primary),
        },
        status: None,
    };

    // Apply the new backup object
    let ps = PatchParams::apply("cntrlr").force();

    let _ = backup_api
        .patch(&snapshot_name, &ps, &Patch::Apply(&backup))
        .await
        .map_err(|e| {
            error!("Error patching backup: {}", e);
            Action::requeue(Duration::from_secs(299))
        })?;

    return Ok(());
}

// to_compact_iso8601 converts a DateTime<Utc> to a compact ISO8601 string
#[instrument]
fn to_compact_iso8601(time: DateTime<Utc>) -> String {
    time.format("%Y%m%d%H%M").to_string()
}

// generate_snapshot_name generates a snapshot name based on the instance name and the current timestamp
#[instrument(fields(trace_id, instance_name = %name))]
fn generate_snapshot_name(name: &str, timestamp: &str) -> String {
    let max_name_len = 54 - timestamp.len() - 1; // Subtract 1 for the hyphen separator
    let truncated_name = if name.len() > max_name_len {
        &name[..max_name_len]
    } else {
        name
    };

    format!("{}-{}", truncated_name, timestamp)
}

// Checks to see if the instance count is changing from 1 -> 2
#[instrument(skip(cdb, cluster) fields(trace_id, instance_name = %cdb.name_any()))]
fn replicas_increasing(cdb: &CoreDB, cluster: &Cluster) -> bool {
    // Desired replicas from the CoreDB object
    let cdb_replicas: i64 = cdb.spec.replicas.into();

    // Current instances from Cluster status
    let current_instances = cluster.status.as_ref().and_then(|s| s.instances);

    // Check if transitioning from 1 to 2
    if matches!((cdb_replicas, current_instances), (2, Some(1))) {
        info!(
            "Instance count is changing from 1 to 2 for instance {}. Desired cdb_replicas: {}, Current instances: {}",
            cdb.name_any(), cdb_replicas, current_instances.unwrap()
        );
        return true;
    } else {
        debug!(
            "No transition from 1 to 2 detected for instance {}. Desired cdb_replicas: {}, Current instances: {}",
            cdb.name_any(), cdb_replicas, current_instances.unwrap()
        );
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        apis::coredb_types::{CoreDB, CoreDBSpec},
        cloudnativepg::{
            backups::{Backup, BackupSpec, BackupStatus},
            clusters::{Cluster, ClusterSpec, ClusterStatus},
        },
    };
    use chrono::{TimeDelta, Utc};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use kube::api::ListMeta;
    #[test]
    fn test_check_cluster_instance_count() {
        // Case 1: Scaling from 1 to 2 instances
        let cdb = CoreDB {
            metadata: ObjectMeta {
                name: Some("test-cluster".to_string()),
                namespace: Some("test".to_string()),
                ..ObjectMeta::default()
            },
            spec: CoreDBSpec {
                replicas: 2,
                ..CoreDBSpec::default()
            },
            status: None,
        };
        let cluster = Cluster {
            metadata: ObjectMeta {
                name: Some("test-cluster".to_string()),
                namespace: Some("test".to_string()),
                ..ObjectMeta::default()
            },
            spec: ClusterSpec {
                instances: 2,
                ..ClusterSpec::default()
            },
            status: Some(ClusterStatus {
                instances: Some(1),
                ..ClusterStatus::default()
            }),
        };
        assert!(replicas_increasing(&cdb, &cluster));

        // Case 2: No scaling, current instances already 2
        let cdb = CoreDB {
            metadata: ObjectMeta {
                name: Some("test-cluster".to_string()),
                namespace: Some("test".to_string()),
                ..ObjectMeta::default()
            },
            spec: CoreDBSpec {
                replicas: 2,
                ..CoreDBSpec::default()
            },
            status: None,
        };
        let cluster = Cluster {
            metadata: ObjectMeta {
                name: Some("test-cluster".to_string()),
                namespace: Some("test".to_string()),
                ..ObjectMeta::default()
            },
            spec: ClusterSpec {
                instances: 2,
                ..ClusterSpec::default()
            },
            status: Some(ClusterStatus {
                instances: Some(2),
                ..ClusterStatus::default()
            }),
        };
        assert!(!replicas_increasing(&cdb, &cluster));

        // Case 3: No scaling, replicas and instances not 2
        let cdb = CoreDB {
            metadata: ObjectMeta {
                name: Some("test-cluster".to_string()),
                namespace: Some("test".to_string()),
                ..ObjectMeta::default()
            },
            spec: CoreDBSpec {
                replicas: 1,
                ..CoreDBSpec::default()
            },
            status: None,
        };
        let cluster = Cluster {
            metadata: ObjectMeta {
                name: Some("test-cluster".to_string()),
                namespace: Some("test".to_string()),
                ..ObjectMeta::default()
            },
            spec: ClusterSpec {
                instances: 1,
                ..ClusterSpec::default()
            },
            status: Some(ClusterStatus {
                instances: Some(1),
                ..ClusterStatus::default()
            }),
        };
        assert!(!replicas_increasing(&cdb, &cluster));
    }

    #[test]
    fn test_has_currently_running_volume_snaps() {
        let now = Utc::now();

        let backup_running = Backup {
            metadata: ObjectMeta {
                creation_timestamp: Some(Time(now - TimeDelta::try_minutes(60).unwrap())),
                ..ObjectMeta::default()
            },
            spec: BackupSpec {
                method: Some(BackupMethod::VolumeSnapshot),
                ..BackupSpec::default()
            },
            status: Some(BackupStatus {
                phase: Some("running".to_string()),
                ..BackupStatus::default()
            }),
        };

        let backup_pending = Backup {
            metadata: ObjectMeta {
                creation_timestamp: Some(Time(now - TimeDelta::try_minutes(60).unwrap())),
                ..ObjectMeta::default()
            },
            spec: BackupSpec {
                method: Some(BackupMethod::VolumeSnapshot),
                ..BackupSpec::default()
            },
            status: Some(BackupStatus {
                phase: Some("pending".to_string()),
                ..BackupStatus::default()
            }),
        };

        let backup_finalizing = Backup {
            metadata: ObjectMeta {
                creation_timestamp: Some(Time(now - TimeDelta::try_minutes(60).unwrap())),
                ..ObjectMeta::default()
            },
            spec: BackupSpec {
                method: Some(BackupMethod::VolumeSnapshot),
                ..BackupSpec::default()
            },
            status: Some(BackupStatus {
                phase: Some("finalizing".to_string()),
                ..BackupStatus::default()
            }),
        };

        let backup_completed = Backup {
            metadata: ObjectMeta {
                creation_timestamp: Some(Time(now - TimeDelta::try_minutes(90).unwrap())),
                ..ObjectMeta::default()
            },
            spec: BackupSpec {
                method: Some(BackupMethod::VolumeSnapshot),
                ..BackupSpec::default()
            },
            status: Some(BackupStatus {
                phase: Some("completed".to_string()),
                ..BackupStatus::default()
            }),
        };

        let backup_no_status = Backup {
            metadata: ObjectMeta {
                creation_timestamp: Some(Time(now - TimeDelta::try_minutes(60).unwrap())),
                ..ObjectMeta::default()
            },
            spec: BackupSpec {
                method: Some(BackupMethod::VolumeSnapshot),
                ..BackupSpec::default()
            },
            status: None,
        };

        // Check if there is a currently running volume snapshot
        let backups_list = ObjectList {
            items: vec![backup_running.clone(), backup_completed.clone()],
            metadata: ListMeta::default(),
        };

        assert!(has_currently_running_volume_snaps(&backups_list, Time(now)));

        // Check if there is a currently pending volume snapshot
        let backups_list = ObjectList {
            items: vec![backup_pending.clone(), backup_completed.clone()],
            metadata: ListMeta::default(),
        };

        assert!(has_currently_running_volume_snaps(&backups_list, Time(now)));

        // Check if there is a currently finishing volume snapshot
        let backups_list = ObjectList {
            items: vec![backup_finalizing, backup_completed.clone()],
            metadata: ListMeta::default(),
        };

        assert!(has_currently_running_volume_snaps(&backups_list, Time(now)));

        // Check if there is a backup with no status field
        let backups_list = ObjectList {
            items: vec![backup_no_status, backup_completed.clone()],
            metadata: ListMeta::default(),
        };

        assert!(has_currently_running_volume_snaps(&backups_list, Time(now)));

        let backups_list = ObjectList {
            items: vec![backup_completed.clone()],
            metadata: ListMeta::default(),
        };

        assert!(!has_currently_running_volume_snaps(
            &backups_list,
            Time(now)
        ));
    }

    #[test]
    fn test_generate_snapshot_name() {
        let now = Utc::now();
        let timestamp = to_compact_iso8601(now);

        // Test case 1: Name fits within 54 characters
        let name1 = "my-snapshot";
        let snapshot_name1 = generate_snapshot_name(name1, &timestamp);
        println!("snapshot_name1: {}", snapshot_name1);
        assert!(snapshot_name1.starts_with(name1));
        assert!(snapshot_name1.ends_with(&timestamp));
        assert!(snapshot_name1.len() <= 54);

        // Test case 2: Name and timestamp together exceed 54 characters
        let name2 = "a-very-long-snapshot-name-that-exceeds-54-characters";
        let snapshot_name2 = generate_snapshot_name(name2, &timestamp);
        let max_name_len = 54 - timestamp.len() - 1; // Subtract 1 for the hyphen separator
        let truncated_name = &name2[..max_name_len];
        println!("snapshot_name2: {}", snapshot_name2);
        assert!(snapshot_name2.starts_with(truncated_name));
        assert!(snapshot_name2.len() <= 54);
        assert_eq!(snapshot_name2, format!("{}-{}", truncated_name, timestamp));
    }
}
