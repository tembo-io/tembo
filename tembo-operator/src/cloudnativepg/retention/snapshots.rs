use crate::{
    apis::coredb_types::CoreDB,
    cloudnativepg::backups::{Backup, BackupMethod},
    snapshots::volumesnapshots_crd::VolumeSnapshot,
};
use chrono::{DateTime, Duration, Utc};
use kube::{
    api::{Api, DeleteParams, ListParams},
    runtime::controller::Action,
    Client as KubeClient, ResourceExt,
};
use tracing::{debug, error, info, instrument, warn};

/// Cleans up old volume snapshots and their associated backups based on retention policy
///
/// # Arguments
///
/// * `cdb` - Reference to the CoreDB instance being reconciled
/// * `client` - Kubernetes client for API operations
/// * `retention_days` - Number of days to retain snapshots before deletion
///
/// # Returns
///
/// Returns `Ok(())` if cleanup was successful, or an `Action` indicating how to handle any errors
///
/// # Error Handling
///
/// - Returns a requeue Action if namespace is missing
/// - Returns a requeue Action if listing backups fails
/// - Individual backup/snapshot deletion failures are logged but don't stop the process
#[instrument(skip(cdb, client), fields(
    instance = %cdb.name_any(),
    namespace = %cdb.namespace().unwrap_or_default(),
    retention_days = %retention_days
))]
pub async fn cleanup_old_volume_snapshots(
    cdb: &CoreDB,
    client: KubeClient,
    retention_days: u64,
) -> Result<(), Action> {
    let name = cdb.name_any();
    let namespace = cdb.metadata.namespace.as_ref().ok_or_else(|| {
        error!("Namespace is empty for instance: {}.", cdb.name_any());
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;
    let backups_api: Api<Backup> = Api::namespaced(client.clone(), namespace);
    let snapshots_api: Api<VolumeSnapshot> = Api::namespaced(client.clone(), namespace);
    let cutoff_time = Utc::now() - Duration::days(retention_days as i64);

    debug!(
        cutoff_time = %cutoff_time,
        "Starting cleanup of old volume snapshots for instance: {}", name
    );

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

    debug!(
        backup_count = backups.len(),
        "Found volume snapshot backups to evaluate for instance: {}", name
    );

    for backup in backups {
        if should_delete_backup(&backup, cutoff_time) {
            delete_backup_and_snapshot(&backups_api, &snapshots_api, &backup, namespace).await?;
        }
    }

    info!("Completed volume snapshot cleanup for instance: {}", name);
    Ok(())
}

/// Determines if a backup should be deleted based on its age
///
/// # Arguments
///
/// * `backup` - The backup to evaluate
/// * `cutoff_time` - The timestamp before which backups should be deleted
///
/// # Returns
///
/// Returns true if the backup is older than the cutoff time and should be deleted
#[instrument(skip(backup, cutoff_time), fields(
    backup_name = %backup.name_any(),
    creation_time = ?backup.metadata.creation_timestamp
))]
fn should_delete_backup(backup: &Backup, cutoff_time: DateTime<Utc>) -> bool {
    // We only need to check timestamp now since we've already filtered for volumeSnapshot method
    backup
        .metadata
        .creation_timestamp
        .as_ref()
        .map(|ts| ts.0 < cutoff_time)
        .unwrap_or(false)
}

/// Deletes a backup and its associated volume snapshot
///
/// # Arguments
///
/// * `backups_api` - The Kubernetes API client for Backup resources
/// * `snapshots_api` - The Kubernetes API client for VolumeSnapshot resources
/// * `backup` - The backup to delete
/// * `namespace` - The namespace containing the resources
///
/// # Returns
///
/// Returns `Ok(())` if deletion was successful, or an `Action` indicating how to handle any errors
#[instrument(skip(backups_api, snapshots_api, backup), fields(
    backup_name = %backup.name_any(),
    namespace = %namespace
))]
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
