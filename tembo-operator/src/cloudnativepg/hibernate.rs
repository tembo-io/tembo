use crate::apis::coredb_types::CoreDB;
use crate::cloudnativepg::cnpg::{get_cluster, get_pooler, get_scheduled_backup};
use crate::cloudnativepg::poolers::Pooler;
use crate::cloudnativepg::scheduledbackups::ScheduledBackup;
use crate::Error;

use crate::{patch_cdb_status_merge, requeue_normal_with_jitter, Context};
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use serde_json::json;

use k8s_openapi::api::apps::v1::Deployment;

use crate::app_service::manager::get_appservice_deployment_objects;
use crate::cloudnativepg::cnpg_utils::{
    get_pooler_instances, patch_cluster_merge, patch_pooler_merge, patch_scheduled_backup_merge,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Resolves hibernation in the Cluster and related services of the CoreDB
///
/// If the cluster is in spec.stop state, this will activate the CNPG hibernation
/// annotation. It also sets the number of replicas to 0 for all related app
/// service deployments in the namespace.
///
/// For the sake of consistency, it also ensures the hibernation annotation is
/// set to "off" when the instance is not stopped and the cluster already exists.
///
/// Returns a normal, jittered requeue when the instance is stopped.
pub async fn reconcile_cluster_hibernation(cdb: &CoreDB, ctx: &Arc<Context>) -> Result<(), Action> {
    info!(
        "Reconciling hibernation for CoreDB instance {}",
        cdb.name_any()
    );
    let name = cdb.name_any();
    let namespace = cdb.namespace().ok_or_else(|| {
        error!("Namespace is not set for CoreDB instance {}", name);
        Action::requeue(Duration::from_secs(300))
    })?;

    // Check if the cluster exists; if not, exit early.
    let cluster = get_cluster(cdb, ctx.clone()).await;
    let cluster = match cluster {
        Some(cluster) => cluster,
        None => {
            warn!("Cluster {} does not exist yet. Proceeding...", name);
            return Ok(());
        }
    };

    let scheduled_backup = get_scheduled_backup(cdb, ctx.clone()).await;
    if scheduled_backup.is_none() {
        warn!(
            "ScheduledBackup {} does not exist or backups are disabled. Proceeding without it...",
            name
        );
    }

    let pooler = get_pooler(cdb, ctx.clone()).await;
    if pooler.is_none() {
        warn!(
            "Pooler {} does not exist or disabled. Proceeding without it...",
            name
        );
    }

    let client = ctx.client.clone();
    let coredbs: Api<CoreDB> = Api::namespaced(client.clone(), &namespace);
    let deployment_api: Api<Deployment> = Api::namespaced(client.clone(), &namespace);
    let ps = PatchParams::apply("patch_merge").force();

    // Along with the CNPG cluster itself, we also need to stop each of the
    // associated app services. We can do this by retrieving a list of depolyments
    // in the cluster and setting their replica count to 0 so they spin down.
    // Conversely, setting it back to 1 if the cluster is started should reverse
    // the process.

    let replicas = if cdb.spec.stop { 0 } else { 1 };
    let replica_patch = json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "spec": {
            "replicas": replicas,
        }
    });

    let deployment_list = match get_appservice_deployment_objects(&client, &namespace, &name).await
    {
        Ok(deployments) => deployments,
        Err(e) => {
            warn!(
                "Could not retrieve deployment list for cluster {}; retrying",
                name
            );
            debug!("Caught error {}", e);
            return Err(requeue_normal_with_jitter());
        }
    };

    // We just need to patch any deployment that has a mismatched replica count.
    // If we're stopped, the deployment should have 0 replicas, or 1 otherwise.
    // We may need to rethink this logic in the future if we ever have deployments
    // that require allow more than 1 active replica.

    for deployment in deployment_list {
        let spec = match deployment.spec {
            Some(spec) => spec,
            None => continue,
        };
        let dep_name = match deployment.metadata.name {
            Some(dep_name) => dep_name,
            None => continue,
        };

        if Some(replicas) == spec.replicas {
            continue;
        }

        match deployment_api
            .patch(&dep_name, &ps, &Patch::Apply(&replica_patch))
            .await
            .map_err(Error::KubeError)
        {
            Ok(_) => {
                debug!(
                    "ns: {}, patched AppService Deployment: {}",
                    &namespace, dep_name
                );
            }
            Err(e) => {
                warn!(
                    "Could not patch deployment {} for cluster {}; retrying",
                    dep_name, name
                );
                debug!("Caught error {}", e);
                return Err(requeue_normal_with_jitter());
            }
        }
    }

    // Build the hibernation patch we want to apply to disable the CNPG cluster.

    let cluster_annotations = cluster.metadata.annotations.unwrap_or_default();
    let hibernation_value = if cdb.spec.stop { "on" } else { "off" };
    let patch_hibernation_annotation = json!({
        "metadata": {
            "annotations": {
                "cnpg.io/hibernation": hibernation_value
            }
        }
    });

    // Update ScheduledBackup if it exists
    if let Err(action) = update_scheduled_backup(&scheduled_backup, cdb, ctx).await {
        warn!(
            "Error updating scheduled backup for {}. Requeuing...",
            cdb.name_any()
        );
        return Err(action);
    }

    // Patch the Pooler cluster resource to match the hibernation state
    if let Err(action) = update_pooler_instances(&pooler, cdb, ctx).await {
        warn!(
            "Error updating pooler instances for {}. Requeuing...",
            cdb.name_any()
        );
        return Err(action);
    }

    // Check the annotation we are about to match was already there
    if let Some(current_hibernation_setting) = cluster_annotations.get("cnpg.io/hibernation") {
        if current_hibernation_setting == hibernation_value {
            debug!(
                "Hibernation annotation of {} already set to '{}', proceeding...",
                name, hibernation_value
            );
            if cdb.spec.stop {
                info!("Fully reconciled stopped instance {}", name);
                return Err(requeue_normal_with_jitter());
            }
            return Ok(());
        }
    }
    patch_cluster_merge(cdb, ctx, patch_hibernation_annotation).await?;
    info!(
        "Toggled hibernation annotation of {} to '{}'",
        name, hibernation_value
    );

    let mut status = cdb.status.clone().unwrap_or_default();
    status.running = !cdb.spec.stop;
    status.pg_postmaster_start_time = None;

    let patch_status = json!({
        "apiVersion": "coredb.io/v1alpha1",
        "kind": "CoreDB",
        "status": status
    });
    patch_cdb_status_merge(&coredbs, &name, patch_status).await?;

    if cdb.spec.stop {
        info!("Fully reconciled stopped instance {}", name);
        return Err(requeue_normal_with_jitter());
    }
    Ok(())
}

async fn update_pooler_instances(
    pooler: &Option<Pooler>,
    cdb: &CoreDB,
    ctx: &Arc<Context>,
) -> Result<(), Action> {
    let name = cdb.name_any();

    match pooler {
        Some(p) => {
            let current_instances = p.spec.instances.unwrap_or(1);
            let desired_instances = get_pooler_instances(cdb);

            if let Some(desired) = desired_instances {
                if current_instances != desired {
                    let patch_pooler_spec = json!({
                        "spec": {
                            "instances": desired,
                        }
                    });

                    match patch_pooler_merge(cdb, ctx, patch_pooler_spec).await {
                        Ok(_) => {
                            info!(
                                "Updated Pooler instances for {} from {} to {}",
                                name, current_instances, desired
                            );
                        }
                        Err(e) => {
                            error!("Failed to update Pooler instances for {}: {:?}", name, e);
                            return Err(requeue_normal_with_jitter());
                        }
                    }
                } else {
                    debug!(
                        "Pooler instances for {} already set to {}. No update needed.",
                        name, current_instances
                    );
                }
            } else {
                warn!(
                    "Could not determine desired instances for Pooler {}. Skipping update.",
                    name
                );
            }
        }
        None => {
            info!(
                "Skipping Pooler operations as it doesn't exist for {}",
                name
            );
        }
    }

    Ok(())
}

async fn update_scheduled_backup(
    scheduled_backup: &Option<ScheduledBackup>,
    cdb: &CoreDB,
    ctx: &Arc<Context>,
) -> Result<(), Action> {
    let name = cdb.name_any();

    if let Some(sb) = scheduled_backup {
        let scheduled_backup_suspend_status = sb.spec.suspend.unwrap_or_default();
        let scheduled_backup_value = cdb.spec.stop;

        if scheduled_backup_suspend_status != scheduled_backup_value {
            let patch_scheduled_backup_spec = json!({
                "spec": {
                    "suspend": scheduled_backup_value
                }
            });

            match patch_scheduled_backup_merge(cdb, ctx, patch_scheduled_backup_spec).await {
                Ok(_) => {
                    info!(
                        "Toggled scheduled backup suspend of {} to '{}'",
                        name, scheduled_backup_value
                    );
                }
                Err(e) => {
                    error!("Failed to update ScheduledBackup for {}: {:?}", name, e);
                    return Err(requeue_normal_with_jitter());
                }
            }
        } else {
            debug!(
                "ScheduledBackup suspend for {} already set to {}. No update needed.",
                name, scheduled_backup_value
            );
        }
    } else {
        info!(
            "Skipping ScheduledBackup operations as it doesn't exist for {}",
            name
        );
    }

    Ok(())
}
