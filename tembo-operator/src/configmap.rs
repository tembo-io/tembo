use crate::Error;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::{
    api::{Api, ObjectMeta, Patch, PatchParams},
    Client,
};
use std::collections::BTreeMap;

use tracing::{debug, error};

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
