use crate::Error;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::{
    api::{Api, ObjectMeta, Patch, PatchParams, PostParams},
    Client,
};
use std::collections::BTreeMap;

use tracing::{debug, error, info};


pub async fn create_configmap_ifnotexist(
    client: Client,
    namespace: &str,
    cm_name: &str,
) -> Result<(), Error> {
    let cm_api: Api<ConfigMap> = Api::namespaced(client, namespace);

    match cm_api.get(cm_name).await {
        Ok(o) => {
            debug!("Configmap {} already exists", o.metadata.name.unwrap());
            return Ok(());
        }
        Err(e) => {
            create_configmap(cm_api, cm_name).await?;
            return Ok(());
        }
    };
}

pub async fn create_configmap(cm_api: Api<ConfigMap>, cm_name: &str) -> Result<(), Error> {
    let cm = ConfigMap {
        metadata: ObjectMeta {
            name: Some(cm_name.to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    let pp = PostParams::default();
    match cm_api.create(&pp, &cm).await {
        Ok(o) => {
            info!("Created empty configmap: {}", o.metadata.name.unwrap());
        }
        Err(e) => {
            error!("Failed to create empty configmap: {}", e);
        }
    };
    Ok(())
}

pub async fn set_configmap(
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

    let patch_params = PatchParams::apply("myapp");
    let patch = Patch::Apply(&cm);

    match cm_api.patch(cm_name, &patch_params, &patch).await {
        Ok(o) => {
            println!("Updated configmap: {}", o.metadata.name.unwrap());
        }
        Err(e) => {
            eprintln!("Failed to update configmap: {}", e);
        }
    };

    let pp = PostParams::default();
    match cm_api.create(&pp, &cm).await {
        Ok(o) => {
            info!("Created empty configmap: {}", o.metadata.name.unwrap());
        }
        Err(e) => {
            error!("Failed to create empty configmap: {}", e);
        }
    };
    Ok(())
}
