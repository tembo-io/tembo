use crate::{apis::coredb_types::CoreDB, Context, Error};
use k8s_openapi::api::core::v1::ConfigMap;
use kube::{
    api::{Api, ObjectMeta, Patch, PatchParams},
    runtime::controller::Action,
    Client, ResourceExt,
};
use std::{collections::BTreeMap, sync::Arc};

use tracing::{debug, error, instrument};

#[instrument(skip(cdb, ctx) fields(trace_id, instance_name = %cdb.name_any()))]
pub async fn reconcile_generic_metrics_configmap(cdb: &CoreDB, ctx: Arc<Context>) -> Result<(), Action> {
    let (custom_metrics_namespace, custom_metrics_name) = match custom_metrics_configmap_settings() {
        Some(value) => value,
        _ => return Ok(()),
    };

    let namespace = cdb.namespace().unwrap();
    let client = ctx.client.clone();
    let configmap_api_dataplane_namespace: Api<ConfigMap> =
        Api::namespaced(client.clone(), &custom_metrics_namespace);

    let configmap_name = format!("{}-custom", cdb.name_any());

    match configmap_api_dataplane_namespace.get(&custom_metrics_name).await {
        Ok(original_configmap) => {
            let data = original_configmap.data.clone().unwrap_or_default();
            match apply_configmap(client, &namespace, &configmap_name, data).await {
                Ok(_) => {
                    debug!("ConfigMap data applied successfully to namespace '{}'", namespace);
                }
                Err(e) => {
                    error!("Failed to apply ConfigMap in namespace '{}': {:?}", namespace, e);
                    return Err(Action::requeue(std::time::Duration::from_secs(300)));
                }
            }
        }
        Err(e) => {
            println!(
                "Failed to get ConfigMap from '{}' namespace: {:?}",
                &custom_metrics_namespace, e
            );
            return Err(Action::requeue(std::time::Duration::from_secs(300)));
        }
    }

    Ok(())
}

pub fn custom_metrics_configmap_settings() -> Option<(String, String)> {
    let custom_metrics_namespace = match std::env::var("CUSTOM_METRICS_CONFIGMAP_NAMESPACE") {
        Ok(namespace) => namespace,
        Err(_) => {
            debug!("CUSTOM_METRICS_NAMESPACE not set, skipping adding custom metrics");
            return None;
        }
    };
    let custom_metrics_name = match std::env::var("CUSTOM_METRICS_CONFIGMAP_NAME") {
        Ok(name) => name,
        Err(_) => {
            debug!("CUSTOM_METRICS_NAME not set, skipping adding custom metrics");
            return None;
        }
    };
    Some((custom_metrics_namespace, custom_metrics_name))
}

pub async fn apply_configmap(
    client: Client,
    namespace: &str,
    cm_name: &str,
    data: BTreeMap<String, String>,
) -> Result<(), Error> {
    let cm_api: Api<ConfigMap> = Api::namespaced(client, namespace);
    let cm = ConfigMap {
        metadata: ObjectMeta {
            name: Some(cm_name.to_string()),
            ..Default::default()
        },
        data: Some(data),
        ..Default::default()
    };

    let patch_params = PatchParams::apply("cntrlr");
    let patch = Patch::Apply(&cm);

    match cm_api.patch(cm_name, &patch_params, &patch).await {
        Ok(o) => {
            debug!("Set configmap: {}", o.metadata.name.unwrap());
        }
        Err(e) => {
            error!("Failed to set configmap: {}", e);
        }
    };
    Ok(())
}
