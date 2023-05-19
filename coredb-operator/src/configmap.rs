use crate::{apis::coredb_types::CoreDB, defaults, patch_cdb_status_merge, Context, Error};
use k8s_openapi::api::core::v1::ConfigMap;
use kube::{
    api::{Api, ObjectMeta, PostParams, Resource},
    Client,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use tracing::{debug, error, info, warn};


pub async fn create_configmap(client: Client, namespace: &str) -> Result<(), Error> {
    let configmaps: Api<ConfigMap> = Api::namespaced(client, namespace);

    let cm = ConfigMap {
        metadata: ObjectMeta {
            name: Some("prom-pg-queries".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    let pp = PostParams::default();
    match configmaps.create(&pp, &cm).await {
        Ok(o) => {
            info!("Created empty configmap: {}", o.metadata.name.unwrap());
        }
        Err(e) => {
            error!("Failed to create empty configmap: {}", e);
        }
    };
    Ok(())
}
