use crate::{
    apis::coredb_types::CoreDB,
    cloudnativepg::backups::{Backup, BackupMethod},
    snapshots::volumesnapshots_crd::VolumeSnapshot,
    Context,
};
use chrono::{DateTime, Duration, Utc};
use kube::{
    api::{Api, DeleteParams, ListParams},
    runtime::controller::Action,
    ResourceExt,
};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Handle the cleanup of old volume snapshots and their associated backups
pub async fn cleanup_old_volume_snapshots(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    retention_days: u64,
) -> Result<(), Action> {
    let client = ctx.client.clone();
    let namespace = cdb.metadata.namespace.as_ref().ok_or_else(|| {
        error!("Namespace is empty for instance: {}.", cdb.name_any());
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;
    let backups_api: Api<Backup> = Api::namespaced(client.clone(), namespace);
    let snapshots_api: Api<VolumeSnapshot> = Api::namespaced(client.clone(), namespace);
    let cutoff_time = Utc::now() - Duration::days(retention_days as i64);

    // List only volume snapshot backups
    let lp = ListParams::default().fields(&format!("spec.method={}", BackupMethod::VolumeSnapshot));

    // List all backups in the namespace
    let backups = backups_api
        .list(&lp)
        .await
        .map_err(|e| {
            error!("Failed to list backups in namespace {}: {}", namespace, e);
            Action::requeue(tokio::time::Duration::from_secs(300))
        })?
        .items;

    for backup in backups {
        if should_delete_backup(&backup, cutoff_time) {
            delete_backup_and_snapshot(&backups_api, &snapshots_api, &backup, namespace).await?;
        }
    }

    Ok(())
}

fn should_delete_backup(backup: &Backup, cutoff_time: DateTime<Utc>) -> bool {
    // We only need to check timestamp now since we've already filtered for volumeSnapshot method
    backup
        .metadata
        .creation_timestamp
        .as_ref()
        .map(|ts| ts.0 < cutoff_time)
        .unwrap_or(false)
}

async fn delete_backup_and_snapshot(
    backups_api: &Api<Backup>,
    snapshots_api: &Api<VolumeSnapshot>,
    backup: &Backup,
    namespace: &str,
) -> Result<(), Action> {
    let backup_name = backup.metadata.name.as_deref().unwrap_or("unknown");

    // Delete the backup first
    match backups_api
        .delete(backup_name, &DeleteParams::default())
        .await
    {
        Ok(_) => {
            info!(
                "Deleted snapshot Backup '{}' for instance '{}', in namespace '{}'",
                backup_name, namespace, namespace
            );
        }
        Err(e) => {
            warn!(
                "Failed to delete snapshot Backup '{}' for instance '{}' in namespace '{}': {}",
                backup_name, namespace, namespace, e
            );
            // If we can't delete the backup, we shouldn't try to delete the snapshot
            return Err(Action::requeue(tokio::time::Duration::from_secs(300)));
        }
    }

    // After successful backup deletion, try to delete the associated snapshot if it exists
    if let Some(status) = &backup.status {
        if let Some(snapshot_name) = &status.backup_name {
            match snapshots_api
                .delete(snapshot_name, &DeleteParams::default())
                .await
            {
                Ok(_) => {
                    info!(
                        "Deleted VolumeSnapshot '{}' for instance '{}' in namespace '{}'",
                        snapshot_name, namespace, namespace
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to delete VolumeSnapshot '{}' for instance '{}' in namespace '{}': {}",
                        snapshot_name, namespace, namespace, e
                    );
                    // We still return Ok since the backup was successfully deleted
                }
            }
        }
    }

    Ok(())
}
