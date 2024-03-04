use crate::{apis::coredb_types::CoreDB, Context, Error, Result};
use k8s_openapi::api::apps::v1::Deployment;
use kube::api::{Api, ListParams, ResourceExt};
use std::sync::Arc;
use tracing::{debug, error};

// Top level function to cleanup all postgres-exporter resources
// this includes the deployment, service and rbac
pub async fn cleanup_postgres_exporter(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    delete_postgres_exporter_deployment(cdb, ctx.clone()).await?;
    crate::service::delete_postgres_exporter_service(cdb, ctx.clone()).await?;
    crate::rbac::cleanup_postgres_exporter_rbac(cdb, ctx.clone()).await?;
    Ok(())
}

// Delete the postgres-exporter Deployment from the cluster
async fn delete_postgres_exporter_deployment(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let deployment_api: Api<Deployment> = Api::namespaced(client, &ns);

    // Define the label selector based on your deployment labels
    let label_selector =
        "app=postgres-exporter,component=metrics,coredb.io/name=".to_owned() + &cdb.name_any();
    let lp = ListParams::default().labels(&label_selector);

    // List deployments with specified labels
    let deployments = deployment_api.list(&lp).await?;

    // Delete the deployment
    for deployment in deployments {
        if let Some(deployment_name) = deployment.metadata.name {
            match deployment_api
                .delete(&deployment_name, &Default::default())
                .await
            {
                Ok(_) => {
                    debug!(
                        "Deleted Deployment: {}, for instance {}",
                        deployment_name,
                        cdb.name_any()
                    );
                }
                Err(e) => {
                    error!(
                        "Error deleting Deployment: {}, for instance {}",
                        e,
                        cdb.name_any()
                    );
                    return Err(Error::KubeError(e));
                }
            }
        }
    }

    Ok(())
}