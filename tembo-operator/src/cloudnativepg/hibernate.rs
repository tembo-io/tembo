use crate::apis::coredb_types::CoreDB;
use crate::cloudnativepg::cnpg::get_cluster;

use crate::{patch_cdb_status_merge, requeue_normal_with_jitter, Context};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use serde_json::json;

use crate::cloudnativepg::cnpg_utils::patch_cluster_merge;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

// Applies hibernation to the Cluster if the CoreDB is stopped, then updates the CoreDB Status.
// Returns a normal, jittered requeue when the instance is stopped.
// When the instance is not stopped and the cluster already exists,
// ensure the hibernation annotation is "off"
pub async fn reconcile_cluster_hibernation(cdb: &CoreDB, ctx: &Arc<Context>) -> Result<(), Action> {
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

    // TODO: conditionally stop all app services' deployments by setting replicas to 0
    // - Add new function to app_service module that returns the names of all deployments
    // - Call that function, loop through each and set replicas to 0

    let cluster_annotations = cluster.metadata.annotations.unwrap_or_default();
    let hibernation_value = if cdb.spec.stop { "on" } else { "off" };
    let patch_hibernation_annotation = json!({
        "metadata": {
            "annotations": {
                "cnpg.io/hibernation": hibernation_value
            }
        }
    });
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

    let client = ctx.client.clone();
    let coredbs: Api<CoreDB> = Api::namespaced(client, &namespace);
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
