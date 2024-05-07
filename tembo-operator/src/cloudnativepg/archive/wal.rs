use crate::{
    apis::coredb_types::CoreDB,
    cloudnativepg::{clusters::ClusterStatusConditionsStatus, cnpg::get_cluster},
    Context,
};
use chrono::{DateTime, Utc};
use kube::{runtime::controller::Action, ResourceExt};
use std::sync::Arc;
use tracing::error;

// Find status of the last time a WAL archive was successful and retrun the date
pub async fn reconcile_last_archive_status(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<Option<DateTime<Utc>>, Action> {
    let name = cdb.name_any();

    let cluster = get_cluster(cdb, ctx.clone()).await;
    match cluster {
        Some(cluster) => {
            if let Some(status) = &cluster.status {
                if let Some(conditions) = &status.conditions {
                    for condition in conditions {
                        if condition.r#type == "ContinuousArchiving"
                            && condition.status == ClusterStatusConditionsStatus::True
                        {
                            let last_transition_time = &condition.last_transition_time;
                            if let Ok(last_transition_time) =
                                DateTime::parse_from_rfc3339(last_transition_time)
                            {
                                return Ok(Some(last_transition_time.with_timezone(&Utc)));
                            }
                        }
                    }
                }
            }
            Ok(None)
        }
        None => {
            error!("Failed to get cluster: {}", &name);
            Err(Action::requeue(tokio::time::Duration::from_secs(300)))
        }
    }
}
