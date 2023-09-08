pub mod database_queries;
pub mod install;
pub mod kubernetes_queries;
pub mod toggle;
pub mod types;

use crate::{
    apis::coredb_types::CoreDB,
    extensions::types::{ExtensionStatus, TrunkInstallStatus},
    Context,
};
use kube::{runtime::controller::Action, Api};
use std::sync::Arc;
use tracing::debug;

/// reconcile extensions between the spec and the database
pub async fn reconcile_extensions(
    coredb: &CoreDB,
    ctx: Arc<Context>,
    _cdb_api: &Api<CoreDB>,
    _name: &str,
) -> Result<(Vec<TrunkInstallStatus>, Vec<ExtensionStatus>), Action> {
    let coredb_name = coredb.metadata.name.clone().expect("CoreDB should have a name");
    debug!("Reconciling trunk installs: {}", coredb_name);
    let trunk_installs = install::reconcile_trunk_installs(coredb, ctx.clone()).await?;
    debug!("Reconciling extension statuses: {}", coredb_name);
    let extension_statuses = toggle::reconcile_extension_toggle_state(coredb, ctx.clone()).await?;
    Ok((trunk_installs, extension_statuses))
}
