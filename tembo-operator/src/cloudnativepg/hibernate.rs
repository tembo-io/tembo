use crate::apis::coredb_types::CoreDB;
use crate::cloudnativepg::cnpg::get_cluster;
use crate::cloudnativepg::cnpg_utils;
use crate::{patch_cdb_status_merge, requeue_normal_with_jitter, Context};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

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
            error!("Cluster {} does not exist yet. Proceeding...", name);
            return Ok(());
        }
    };

    // TODO: conditionally stop all app services' deployments by setting replicas to 0
    // - Add new function to app_service module that returns the names of all deployments
    // - Call that function, loop through each and set replicas to 0

    let cluster_annotations = cluster.metadata.annotations.unwrap_or_default();
    patch_hibernation(cdb, cluster_annotations, cdb.spec.stop, ctx, &name).await?;

    if !cdb.spec.stop {
        return Ok(());
    }
    let mut status = cdb.status.clone().unwrap_or_default();
    status.running = false;
    status.pg_postmaster_start_time = None;

    let client = ctx.client.clone();
    let coredbs: Api<CoreDB> = Api::namespaced(client, &namespace);
    let patch_status = json!({
        "apiVersion": "coredb.io/v1alpha1",
        "kind": "CoreDB",
        "status": status
    });
    patch_cdb_status_merge(&coredbs, &name, patch_status).await?;

    info!("Fully reconciled stopped instance {}", name);
    Err(requeue_normal_with_jitter())
}

// Function to patch the hibernation status of a cluster.
async fn patch_hibernation(
    cdb: &CoreDB,
    current_annotations: BTreeMap<String, String>,
    on: bool,
    ctx: &Arc<Context>,
    name: &str,
) -> Result<(), Action> {
    let hibernation_value = if on { "on" } else { "off" };
    let patch = json!({
        "metadata": {
            "annotations": {
                "cnpg.io/hibernation": hibernation_value
            }
        }
    });
    // Check the annotation we are about to match was already there
    if let Some(annot) = current_annotations.get("cnpg.io/hibernation") {
        if annot == hibernation_value {
            return Ok(());
        }
    }
    cnpg_utils::patch_cluster_merge(cdb, ctx, patch).await?;
    info!(
        "Toggled hibernation annotation of {} to '{}'",
        name, hibernation_value
    );
    Ok(())
}
