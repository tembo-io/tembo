pub mod database_queries;
pub mod install;
pub mod kubernetes_queries;
pub mod toggle;
pub mod types;

use crate::{
    apis::coredb_types::CoreDB,
    extensions::types::{ExtensionStatus, TrunkInstallStatus},
    is_postgres_ready, Context,
};
use kube::{
    runtime::{controller::Action, wait::Condition},
    Api, ResourceExt,
};
use std::{sync::Arc, time::Duration};
use tracing::debug;

/// reconcile extensions between the spec and the database
pub async fn reconcile_extensions(
    coredb: &CoreDB,
    ctx: Arc<Context>,
    _cdb_api: &Api<CoreDB>,
    _name: &str,
) -> Result<(Vec<TrunkInstallStatus>, Vec<ExtensionStatus>), Action> {
    // Trunk installs do not require postgres is ready
    let coredb_name = coredb.name_any();
    debug!("Reconciling trunk installs: {}", coredb_name);
    let trunk_installs = install::reconcile_trunk_installs(coredb, ctx.clone()).await?;

    let primary_pod_cnpg = coredb.primary_pod_cnpg(ctx.client.clone()).await?;

    if !is_postgres_ready().matches_object(Some(&primary_pod_cnpg)) {
        debug!("Did not find postgres ready, waiting a short period");
        return Err(Action::requeue(Duration::from_secs(5)));
    }

    // Toggles require postgres is ready
    debug!("Reconciling extension statuses: {}", coredb_name);
    let extension_statuses = toggle::reconcile_extension_toggle_state(coredb, ctx.clone()).await?;
    Ok((trunk_installs, extension_statuses))
}
