use crate::{
    apis::coredb_types::CoreDB,
    cloudnativepg::{clusters::Cluster, cnpg::does_cluster_exist},
    extensions::database_queries::is_not_restarting,
    patch_cdb_status_merge, Context, RESTARTED_AT,
};
use kube::{
    api::{Patch, PatchParams},
    runtime::controller::Action,
    Api, ResourceExt,
};
use serde_json::json;
use std::sync::Arc;
use tokio::time::Duration;
use tracing::{debug, error, info, instrument, warn};

// restart_and_wait_for_restart is a synchronous function that takes a CNPG cluster adds the restart annotation
// and waits for the restart to complete.
#[instrument(skip(cdb, ctx, prev_cluster), fields(trace_id, instance_name = %cdb.name_any()))]
pub(crate) async fn restart_and_wait_for_restart(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    prev_cluster: Option<&Cluster>,
) -> Result<(), Action> {
    // Check if prev_cluster is None, if so return early
    if prev_cluster.is_none() {
        warn!("No previous cluster found for {}", cdb.name_any());
        return Ok(());
    }

    let Some(cdb_restarted_at) = cdb.annotations().get(RESTARTED_AT) else {
        // No need to update the annotation if it's not present in the CoreDB
        warn!("No restart annotation found for {}", cdb.name_any());
        return Ok(());
    };

    // Get the previous value of the annotation, if any
    let previous_restarted_at =
        prev_cluster.and_then(|cluster| cluster.annotations().get(RESTARTED_AT));

    let restart_annotation_updated = previous_restarted_at != Some(cdb_restarted_at);
    debug!(
        "Restart annotation updated: {} for instance: {}",
        restart_annotation_updated,
        cdb.name_any()
    );

    if restart_annotation_updated {
        warn!(
            "Restarting instance: {} with restart annotation: {}",
            cdb.name_any(),
            cdb_restarted_at
        );

        let restart_patch = json!({
            "metadata": {
                "annotations": {
                    RESTARTED_AT: cdb_restarted_at,
                }
            }
        });

        patch_cluster_merge(cdb, &ctx, restart_patch).await?;
        update_coredb_status(cdb, &ctx, false).await?;

        info!(
            "Updated status.running to false in {}, requeuing 10 seconds",
            &cdb.name_any()
        );

        let restart_complete_time = is_not_restarting(cdb, ctx.clone(), "postgres").await?;

        info!(
            "Restart time is {:?} for instance: {}",
            restart_complete_time,
            &cdb.name_any()
        );
    }

    let cdb_api: Api<CoreDB> =
        Api::namespaced(ctx.client.clone(), cdb.metadata.namespace.as_ref().unwrap());
    let coredb_status = cdb_api.get(&cdb.name_any()).await.map_err(|e| {
        error!("Error retrieving CoreDB status: {}", e);
        Action::requeue(Duration::from_secs(300))
    })?;

    if let Some(status) = coredb_status.status {
        if !status.running {
            update_coredb_status(cdb, &ctx, true).await?;
            info!(
                "Updated CoreDB status.running to true for instance: {}",
                &cdb.name_any()
            );
        }
    }

    Ok(())
}

#[instrument(skip(cdb, ctx), fields(trace_id, instance_name = %cdb.name_any(), running = %running))]
async fn update_coredb_status(
    cdb: &CoreDB,
    ctx: &Arc<Context>,
    running: bool,
) -> Result<(), Action> {
    let name = cdb.name_any();
    let namespace = cdb.metadata.namespace.as_ref().ok_or_else(|| {
        error!("Namespace is empty for instance: {}.", name);
        Action::requeue(Duration::from_secs(300))
    })?;

    let cdb_api: Api<CoreDB> = Api::namespaced(ctx.client.clone(), namespace);
    patch_cdb_status_merge(
        &cdb_api,
        &name,
        json!({
            "status": {
                "running": running
            }
        }),
    )
    .await
}

// patch_cluster_merge takes a CoreDB, Cluster and serde_json::Value and patch merges the Cluster with the new spec
#[instrument(skip(cdb, ctx), fields(trace_id, instance_name = %cdb.name_any(), patch = %patch))]
async fn patch_cluster_merge(
    cdb: &CoreDB,
    ctx: &Arc<Context>,
    patch: serde_json::Value,
) -> Result<(), Action> {
    let name = cdb.name_any();
    let namespace = cdb.metadata.namespace.as_ref().ok_or_else(|| {
        error!("Namespace is empty for instance: {}.", name);
        Action::requeue(Duration::from_secs(300))
    })?;

    let cluster_api: Api<Cluster> = Api::namespaced(ctx.client.clone(), namespace);
    let pp = PatchParams::apply("patch_merge");
    let _ = cluster_api
        .patch(&name, &pp, &Patch::Merge(&patch))
        .await
        .map_err(|e| {
            error!("Error patching cluster: {}", e);
            Action::requeue(Duration::from_secs(300))
        });

    info!("Patched Cluster for instance: {}", &name);

    Ok(())
}

// cdb: the CoreDB object
// maybe_cluster, Option<Cluster> of the current CNPG cluster, if it exists
// new_spec: the new Cluster spec to be applied
#[instrument(skip(cdb, maybe_cluster, new_spec), fields(trace_id, instance_name = %cdb.name_any()))]
pub(crate) fn update_restarted_at(
    cdb: &CoreDB,
    maybe_cluster: Option<&Cluster>,
    new_spec: &mut Cluster,
) -> bool {
    let Some(cdb_restarted_at) = cdb.annotations().get(RESTARTED_AT) else {
        // No need to update the annotation if it's not present in the CoreDB
        return false;
    };

    // Remember the previous value of the annotation, if any
    let previous_restarted_at =
        maybe_cluster.and_then(|cluster| cluster.annotations().get(RESTARTED_AT));

    // Forward the `restartedAt` annotation from CoreDB over to the CNPG cluster,
    // does not matter if changed or not.
    new_spec
        .metadata
        .annotations
        .as_mut()
        .map(|cluster_annotations| {
            cluster_annotations.insert(RESTARTED_AT.into(), cdb_restarted_at.to_owned())
        });

    let restart_annotation_updated = previous_restarted_at != Some(cdb_restarted_at);

    if restart_annotation_updated {
        let name = new_spec.metadata.name.as_deref().unwrap_or("unknown");
        info!("restartAt changed for cluster {name}, setting to {cdb_restarted_at}.");
    }

    restart_annotation_updated
}

// patch_cluster is a async function that takes a CNPG cluster and patch applys it with the new spec
#[instrument(skip(cdb, ctx, cluster) fields(trace_id, instance_name = %cdb.name_any()))]
pub(crate) async fn patch_cluster(
    cluster: &Cluster,
    ctx: Arc<Context>,
    cdb: &CoreDB,
) -> Result<(), Action> {
    let name = cdb.name_any();
    let namespace = cdb.metadata.namespace.as_ref().ok_or_else(|| {
        error!("Namespace is empty for instance: {}.", name);
        Action::requeue(tokio::time::Duration::from_secs(300))
    })?;

    // Setup patch parameters
    let pp = PatchParams::apply("cntrlr").force();

    // Setup cluster API
    let api: Api<Cluster> = Api::namespaced(ctx.client.clone(), namespace);

    info!("Applying Cluster for instance: {}", &name);
    let _o = api
        .patch(&name, &pp, &Patch::Apply(&cluster))
        .await
        .map_err(|e| {
            error!("Error patching Cluster: {}", e);
            Action::requeue(Duration::from_secs(300))
        })?;

    Ok(())
}

// is_image_updated function takes a CoreDB, Context and a Cluster and checks to see if the image needs to be updated
#[instrument(skip(cdb, ctx, prev_cluster), fields(trace_id, instance_name = %cdb.name_any()))]
pub(crate) async fn is_image_updated(
    cdb: &CoreDB,
    ctx: Arc<Context>,
    prev_cluster: Option<&Cluster>,
) -> Result<(), Action> {
    // Check if prev_cluster is None, if so return early
    if prev_cluster.is_none() {
        warn!("No previous cluster found for {}", cdb.name_any());
        return Ok(());
    }

    // Check if the image is being updated, check prev_cluster spec.imageName if it's different than what's in cdb.spec.image
    if let Some(prev_cluster) = prev_cluster {
        let prev_image = prev_cluster.spec.image_name.as_deref();
        let new_image = cdb.spec.image.as_str();

        if let Some(prev_image) = prev_image {
            if prev_image != new_image {
                warn!(
                    "Image updated for instance: {} from {} to {}",
                    cdb.name_any(),
                    prev_image,
                    new_image
                );

                // Create JSON Patch
                let patch = json!({
                    "spec": {
                        "imageName": new_image
                    }
                });

                // Update Cluster with patch
                patch_cluster_merge(cdb, &ctx, patch).await?;
            }
        }
    }

    Ok(())
}

// check_cluster_hibernation_status is a async function that takes a CoreDB and Context and checks if the Cluster
// API exists and hibernating is set or not set.  It will return a boolean value (false by default).
pub(crate) async fn check_cluster_hibernation_status(
    cdb: &CoreDB,
    ctx: &Arc<Context>,
) -> Result<bool, Action> {
    let name = cdb.name_any();
    // Check if the cluster exists; if not, exit early.
    if !does_cluster_exist(cdb, ctx.clone()).await? {
        debug!("Cluster does not exist for instance: {}", name);
        return Ok(false);
    }

    // Cluster exists; let's check the status of hibernating.
    let hibernating = get_hibernate_status(cdb, ctx).await?;

    // Decide on the hibernation status and possibly patch the cluster.
    match (hibernating, cdb.spec.stop) {
        (true, false) => {
            // If incorrectly hibernating, patch to deactivate.
            patch_hibernation_status(cdb, ctx, false, &name).await?;
            update_coredb_status(cdb, ctx, true).await?;
            Ok(true)
        }
        (false, true) => {
            // If should be hibernating but isn't, patch to activate.
            patch_hibernation_status(cdb, ctx, true, &name).await?;
            Ok(false)
        }
        _ => Ok(hibernating),
    }
}

// Function to patch the hibernation status of a cluster.
async fn patch_hibernation_status(
    cdb: &CoreDB,
    ctx: &Arc<Context>,
    hibernation_status: bool,
    name: &str,
) -> Result<bool, Action> {
    let hibernation_value = if hibernation_status { "on" } else { "off" };
    let patch = json!({
        "metadata": {
            "annotations": {
                "cnpg.io/hibernation": hibernation_value
            }
        }
    });

    info!(
        "Changing hibernation status for {} to '{}'",
        name, hibernation_value
    );
    patch_cluster_merge(cdb, ctx, patch).await?;

    Ok(hibernation_status)
}

/// Determine the hibernate state of the CNPG cluster resources
///
/// This function will only return true if the cnpg.io/hibernation annotation
/// is set to "on". Otherwise the assumption is that the annotation is not set
/// at all or is set to some other value, in which case CNPG will not hibernate.
#[instrument(skip(cdb, ctx), fields(trace_id, instance_name = %cdb.name_any()))]
pub async fn get_hibernate_status(cdb: &CoreDB, ctx: &Arc<Context>) -> Result<bool, Action> {
    let instance_name = cdb.metadata.name.as_deref().unwrap_or_default();
    let namespace = cdb.namespace().ok_or_else(|| {
        error!("Namespace is not set for CoreDB instance {}", instance_name);
        Action::requeue(Duration::from_secs(300))
    })?;

    let cluster: Api<Cluster> = Api::namespaced(ctx.client.clone(), &namespace);
    let co = cluster.get(instance_name).await.map_err(|e| {
        error!("Error getting cluster: {}", e);
        Action::requeue(Duration::from_secs(300))
    })?;

    let hibernate = match co.metadata.annotations {
        Some(ann) => ann
            .get("cnpg.io/hibernation")
            .map_or(false, |state| state == "on"),
        None => {
            info!("Cluster Status for {} is not set", instance_name);
            return Ok(false);
        }
    };

    Ok(hibernate)
}

// check_cluster_status will check if the Cluster is running or not and verify the actual status to patch the
// CoreDB status.running field.
// pub(crate) async fn check_cluster_status(cdb: &CoreDB, ctx: &Arc<Context>) -> Result<bool, Action> {
//     let name = cdb.name_any();
//     let namespace = cdb.namespace().ok_or_else(|| {
//         error!("Namespace is not set for CoreDB instance {}", name);
//         Action::requeue(Duration::from_secs(300))
//     })?;

//     // Check if the cluster exists; if not, exit early.
//     if !does_cluster_exist(cdb, ctx.clone()).await? {
//         debug!("Cluster does not exist for instance: {}", name);
//         return Ok(false);
//     }

//     // if cluster exists, check the status.conditions for the cluster status
//     let cluster_api: Api<Cluster> = Api::namespaced(ctx.client.clone(), &namespace);
//     let cluster = cluster_api.get(&name).await.map_err(|e| {
//         error!("Error getting cluster: {}", e);
//         Action::requeue(Duration::from_secs(300))
//     })?;

//     let mut is_cluster_ready = false;
//     let mut is_hibernated_present = false;
//     let mut is_hibernated = false;

//     if let Some(status) = &cluster.status {
//         if let Some(conditions) = &status.conditions {
//             for condition in conditions {
//                 if condition.reason == "ClusterIsReady"
//                     && condition.status == ClusterStatusConditionsStatus::True
//                 {
//                     is_cluster_ready = true;
//                     debug!("Cluster '{}' is ready.", name);
//                 }
//                 if condition.reason == "Hibernated" {
//                     is_hibernated_present = true;
//                     is_hibernated = condition.status == ClusterStatusConditionsStatus::True;
//                     debug!(
//                         "Hibernated status for '{}' is '{:?}'.",
//                         name, condition.status
//                     );
//                 }
//             }
//         }
//     }

//     // Return true if ClusterIsReady is true and either Hibernated is false or missing.
//     Ok(is_cluster_ready && (!is_hibernated_present || !is_hibernated))
// }
