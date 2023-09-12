use k8s_openapi::api::core::v1::ConfigMap;
use kube::{runtime::controller::Action, Api, Client};
use std::{collections::BTreeMap, env, time::Duration};

use crate::configmap::apply_configmap;
use tracing::log::error;


const DEFAULT_TRUNK_REGISTRY_DOMAIN: &str = "registry.pgtrunk.io";

// One configmap per namespace
// multiple DBs in the same namespace can share the same configmap
const TRUNK_CONFIGMAP_NAME: &str = "trunk-metadata";

pub async fn extensions_that_require_load(client: Client, namespace: &str) -> Result<Vec<String>, Action> {
    let cm_api: Api<ConfigMap> = Api::namespaced(client, namespace);

    // Get the ConfigMap
    let cm = match cm_api.get(TRUNK_CONFIGMAP_NAME).await {
        Ok(configmap) => configmap,
        Err(_) => {
            error!("Failed to get trunk configmap in namespace {}", namespace);
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };
    if let Some(data) = cm.data {
        if let Some(libraries_str) = data.get("libraries") {
            let libraries: Vec<String> = libraries_str.split(',').map(|s| s.to_string()).collect();
            Ok(libraries)
        } else {
            error!(
                "Invalid content of trunk metadata configmap in namespace {}",
                namespace
            );
            Err(Action::requeue(Duration::from_secs(300)))
        }
    } else {
        error!("No data in trunk metadata configmap in namespace {}", namespace);
        Err(Action::requeue(Duration::from_secs(300)))
    }
}

pub async fn reconcile_trunk_configmap(client: Client, namespace: &str) -> Result<(), Action> {
    let libraries = match requires_load_list_from_trunk().await {
        Ok(libraries) => libraries,
        Err(e) => {
            error!("Failed to update extensions libraries list from trunk: {:?}", e);
            let cm_api: Api<ConfigMap> = Api::namespaced(client.clone(), namespace);
            match cm_api.get(TRUNK_CONFIGMAP_NAME).await {
                Ok(_) => {
                    // If the configmap is already present, we can just log the error and continue
                    return Ok(());
                }
                Err(_e) => {
                    // If the configmap is not already present, then we should requeue the request
                    // as an unexpected error.
                    return Err(Action::requeue(Duration::from_secs(300)));
                }
            }
        }
    };

    let mut data = BTreeMap::new();
    data.insert("libraries".to_string(), libraries.join(","));

    match apply_configmap(client, namespace, TRUNK_CONFIGMAP_NAME, data).await {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("Failed to update trunk configmap: {:?}", e);
            Err(Action::requeue(Duration::from_secs(300)))
        }
    }
}

async fn requires_load_list_from_trunk() -> Result<Vec<String>, TrunkError> {
    let domain =
        env::var("TRUNK_REGISTRY_DOMAIN").unwrap_or_else(|_| DEFAULT_TRUNK_REGISTRY_DOMAIN.to_string());
    let url = format!("https://{}/extensions/libraries", domain);

    let response = reqwest::get(&url).await?;

    if response.status().is_success() {
        let response_body = response.text().await?;
        let libraries: Vec<String> = serde_json::from_str(&response_body)?;
        Ok(libraries)
    } else {
        error!(
            "Failed to update extensions libraries list from trunk: {}",
            response.status()
        );
        Err(TrunkError::ConfigMapApplyError)
    }
}

// Define error type
#[derive(Debug, thiserror::Error)]
pub enum TrunkError {
    #[error("Failed to update extensions libraries list from trunk: {0}")]
    NetworkFailure(#[from] reqwest::Error),
    #[error("Failed to parse extensions libraries list from trunk: {0}")]
    ParsingIssue(#[from] serde_json::Error),
    #[error("Failed to apply trunk configmap")]
    ConfigMapApplyError,
}
