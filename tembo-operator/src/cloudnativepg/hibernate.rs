use crate::apis::coredb_types::CoreDB;
use crate::cloudnativepg::cnpg::does_cluster_exist;
use crate::cloudnativepg::cnpg_utils;
use crate::{patch_cdb_status_merge, requeue_normal_with_jitter, Context};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

// Applies hibernation to the Cluster if the CoreDB is stopped, then updates the CoreDB Status.
// Returns a normal, jittered requeue when the instance is stopped.
// If the CoreDB is not stopped or does not exist, this takes no operation.
// Resuming a database from stopped should not be handled in this function, as the existing
// code will make sure to create all the resources it needs.
pub async fn reconcile_cluster_hibernation(cdb: &CoreDB, ctx: &Arc<Context>) -> Result<(), Action> {
    let name = cdb.name_any();
    let namespace = cdb.namespace().ok_or_else(|| {
        error!("Namespace is not set for CoreDB instance {}", name);
        Action::requeue(Duration::from_secs(300))
    })?;

    if !cdb.spec.stop {
        debug!("Cluster {} is not stopped, taking no action...", name);
        return Ok(());
    }
    // Check if the cluster exists; if not, exit early.
    // We should allow the rest of the reconcile loop
    // to run, so we only apply hibernation after a
    // cluster already exists.
    if !does_cluster_exist(cdb, ctx.clone()).await? {
        warn!(
            "Cluster does not exist for stopped instance {}, proceeding...",
            name
        );
        return Ok(());
    }

    // TODO: stop all app services' deployments by setting replicas to 0
    // - Add new function to app_service module that returns the names of all deployments
    // - Call that function, loop through each and set replicas to 0

    patch_hibernation_on(cdb, ctx, &name).await?;

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
async fn patch_hibernation_on(cdb: &CoreDB, ctx: &Arc<Context>, name: &str) -> Result<(), Action> {
    let patch = json!({
        "metadata": {
            "annotations": {
                "cnpg.io/hibernation": "on"
            }
        }
    });
    cnpg_utils::patch_cluster_merge(cdb, ctx, patch).await?;
    info!("Ensured hibernation enabled for {}", name);
    Ok(())
}
