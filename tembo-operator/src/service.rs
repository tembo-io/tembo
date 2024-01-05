use crate::{apis::coredb_types::CoreDB, Context, Error};
use k8s_openapi::api::core::v1::Service;
use kube::{api::ListParams, Api, ResourceExt};
use std::sync::Arc;
use tracing::{debug, error};

// Delete the postgres-exporter service from the cluster
pub async fn delete_postgres_exporter_service(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<(), Error> {
    let client = ctx.client.clone();
    let ns = cdb.namespace().unwrap();
    let service_api: Api<Service> = Api::namespaced(client, &ns);

    // Define the label selector based on your service labels
    let label_selector =
        "app=postgres-exporter,component=metrics,coredb.io/name=".to_owned() + &cdb.name_any();
    let lp = ListParams::default().labels(&label_selector);

    // List services with specified labels
    let services = service_api.list(&lp).await?;

    // Delete the service
    for service in services {
        if let Some(service_name) = service.metadata.name {
            match service_api.delete(&service_name, &Default::default()).await {
                Ok(_) => {
                    debug!(
                        "Deleted Service: {}, for instance {}",
                        service_name,
                        cdb.name_any()
                    );
                }
                Err(e) => {
                    error!(
                        "Error deleting Service: {}, for instance {}",
                        e,
                        cdb.name_any()
                    );
                    return Err(Error::KubeError(e));
                }
            }
        } else {
            println!("Found a service without a name, skipping...");
        }
    }

    Ok(())
}

// #[instrument(skip(cdb, ctx), fields(instance_name = %cdb.name_any()))]
// pub async fn reconcile_prometheus_exporter_service(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Error> {
//     let client = ctx.client.clone();
//     let ns = cdb.namespace().unwrap();
//     let name = cdb.name_any() + "-metrics";
//     let svc_api: Api<Service> = Api::namespaced(client, &ns);
//     let oref = cdb.controller_owner_ref(&()).unwrap();
//
//     if !(cdb.spec.postgresExporterEnabled) {
//         // check if service exists and delete it
//         let _o = svc_api.delete(&name, &Default::default()).await;
//         return Ok(());
//     }
//
//     let mut selector_labels: BTreeMap<String, String> = BTreeMap::new();
//     selector_labels.insert("app".to_owned(), "postgres-exporter".to_string());
//     selector_labels.insert("coredb.io/name".to_owned(), cdb.name_any());
//     selector_labels.insert("component".to_owned(), "metrics".to_string());
//
//     let mut labels = selector_labels.clone();
//     labels.insert("component".to_owned(), "metrics".to_owned());
//
//     let metrics_svc: Service = Service {
//         metadata: ObjectMeta {
//             name: Some(name.to_owned()),
//             namespace: Some(ns.to_owned()),
//             labels: Some(labels),
//             owner_references: Some(vec![oref]),
//             ..ObjectMeta::default()
//         },
//         spec: Some(ServiceSpec {
//             ports: Some(vec![ServicePort {
//                 port: 80,
//                 name: Some("metrics".to_string()),
//                 target_port: Some(IntOrString::String("metrics".to_string())),
//                 ..ServicePort::default()
//             }]),
//             selector: Some(selector_labels),
//             ..ServiceSpec::default()
//         }),
//         ..Service::default()
//     };
//
//     let ps = PatchParams::apply("cntrlr").force();
//     let _o = svc_api
//         .patch(&name, &ps, &Patch::Apply(&metrics_svc))
//         .await
//         .map_err(Error::KubeError)?;
//
//     Ok(())
// }
