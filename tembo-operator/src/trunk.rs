use k8s_openapi::api::core::v1::ConfigMap;
use kube::{runtime::controller::Action, Api, Client};
use lazy_static::lazy_static;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::Display;
use std::ops::Not;
use std::{collections::BTreeMap, env, time::Duration};

use crate::configmap::apply_configmap;
use tracing::error;
use utoipa::ToSchema;

const DEFAULT_TRUNK_REGISTRY_DOMAIN: &str = "registry.pgtrunk.io";

// One configmap per namespace
// multiple DBs in the same namespace can share the same configmap
const TRUNK_CONFIGMAP_NAME: &str = "trunk-metadata";

#[derive(Debug, Clone, Copy)]
pub enum Version<'a> {
    TrunkProject(&'a str),
    Extension(&'a str),
}

impl Display for Version<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = match self {
            Version::TrunkProject(v) => v,
            Version::Extension(v) => v,
        };

        f.write_str(version)
    }
}

pub struct ExtensionRequiresLoad {
    pub name: String,
    pub library_name: String,
}

// TODO(ianstanton) We can publish this as a crate library and use it in other projects, such as Trunk CLI and Registry
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema, JsonSchema)]
pub struct TrunkProjectMetadata {
    pub name: String,
    pub description: Option<String>,
    pub documentation_link: Option<String>,
    pub repository_link: Option<String>,
    pub version: String,
    pub postgres_versions: Option<Vec<i32>>,
    pub extensions: Vec<TrunkExtensionMetadata>,
    pub downloads: Option<Vec<TrunkDownloadMetadata>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema, JsonSchema)]
pub struct TrunkExtensionMetadata {
    pub extension_name: String,
    pub version: String,
    pub trunk_project_name: String,
    pub dependencies_extension_names: Option<Vec<String>>,
    pub loadable_libraries: Option<Vec<TrunkLoadableLibrariesMetadata>>,
    pub configurations: Option<Vec<String>>,
    pub control_file: TrunkControlFileMetadata,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema, JsonSchema)]
pub struct TrunkDownloadMetadata {
    pub link: String,
    pub pg_version: i32,
    pub platform: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema, JsonSchema)]
pub struct TrunkLoadableLibrariesMetadata {
    pub library_name: String,
    pub requires_restart: bool,
    pub priority: i32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, ToSchema, JsonSchema)]
pub struct TrunkControlFileMetadata {
    pub absent: bool,
    pub content: Option<String>,
}

// This is a place to configure specific exceptions before
// Trunk handles everything.
// In terms of extensions that require load, we need to know
// the library name in some cases where the extension name
// and the library name do not match.
// https://tembo.io/blog/four-types-of-extensions#load
lazy_static! {
    pub static ref EXTRA_EXTENSIONS_REQUIRE_LOAD: Vec<ExtensionRequiresLoad> = {
        let mut extra_extensions_that_require_load = Vec::new();
        let pg_partman = ExtensionRequiresLoad {
            name: "pg_partman".to_string(),
            library_name: "pg_partman_bgw".to_string(),
        };
        extra_extensions_that_require_load.push(pg_partman);
        extra_extensions_that_require_load
    };
}

pub async fn extensions_that_require_load(
    client: Client,
    namespace: &str,
) -> Result<BTreeMap<String, String>, Action> {
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
            // Currently, all extensions returned from the trunk /extensions/libraries
            // require load and have exact name match of the library name to the extension name
            let mut libraries_map = BTreeMap::new();
            for library in libraries {
                libraries_map.insert(library.clone(), library);
            }
            // Add any extra extensions that require load
            for extra_extension in EXTRA_EXTENSIONS_REQUIRE_LOAD.iter() {
                libraries_map.insert(
                    extra_extension.name.clone(),
                    extra_extension.library_name.clone(),
                );
            }
            Ok(libraries_map)
        } else {
            error!(
                "Invalid content of trunk metadata configmap in namespace {}",
                namespace
            );
            Err(Action::requeue(Duration::from_secs(300)))
        }
    } else {
        error!(
            "No data in trunk metadata configmap in namespace {}",
            namespace
        );
        Err(Action::requeue(Duration::from_secs(300)))
    }
}

pub async fn reconcile_trunk_configmap(client: Client, namespace: &str) -> Result<(), Action> {
    let libraries = match requires_load_list_from_trunk().await {
        Ok(libraries) => libraries,
        Err(e) => {
            error!(
                "Failed to update extensions libraries list from trunk: {:?}",
                e
            );
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

// TODO(ianstanton) This information is now available in the trunk project metadata. We should fetch it from there
//  instead
async fn requires_load_list_from_trunk() -> Result<Vec<String>, TrunkError> {
    let domain = env::var("TRUNK_REGISTRY_DOMAIN")
        .unwrap_or_else(|_| DEFAULT_TRUNK_REGISTRY_DOMAIN.to_string());
    let url = format!("https://{}/extensions/libraries", domain);

    let response = reqwest::get(&url).await?;

    if response.status().is_success() {
        let libraries = response.json().await?;
        Ok(libraries)
    } else {
        error!(
            "Failed to update extensions libraries list from trunk: {}",
            response.status()
        );
        Err(TrunkError::ConfigMapApplyError)
    }
}

// Get all trunk projects
pub async fn get_trunk_projects() -> Result<Vec<TrunkProjectMetadata>, TrunkError> {
    let domain = env::var("TRUNK_REGISTRY_DOMAIN")
        .unwrap_or_else(|_| DEFAULT_TRUNK_REGISTRY_DOMAIN.to_string());
    let url = format!("https://{}/api/v1/trunk-projects", domain);

    let response = reqwest::get(&url).await?;

    if response.status().is_success() {
        let project_metadata: Vec<TrunkProjectMetadata> = response.json().await?;
        Ok(project_metadata)
    } else {
        error!("Failed to fetch all trunk projects: {}", response.status());
        Err(TrunkError::NetworkFailure(
            response.error_for_status().unwrap_err(),
        ))
    }
}

// Get all trunk project names
pub async fn get_trunk_project_names() -> Result<Vec<String>, TrunkError> {
    let domain = env::var("TRUNK_REGISTRY_DOMAIN")
        .unwrap_or_else(|_| DEFAULT_TRUNK_REGISTRY_DOMAIN.to_string());
    let url = format!("https://{}/api/v1/trunk-projects", domain);

    let response = reqwest::get(&url).await?;

    if response.status().is_success() {
        let project_metadata: Vec<TrunkProjectMetadata> = response.json().await?;
        let project_names = project_metadata
            .into_iter()
            .map(|project_metadata| project_metadata.name)
            .collect();
        Ok(project_names)
    } else {
        error!("Failed to fetch all trunk projects: {}", response.status());
        Err(TrunkError::NetworkFailure(
            response.error_for_status().unwrap_err(),
        ))
    }
}

// Get the latest metadata entries for a given Trunk project
async fn get_latest_trunk_project_metadata(
    trunk_project: &str,
) -> Result<TrunkProjectMetadata, TrunkError> {
    let domain = env::var("TRUNK_REGISTRY_DOMAIN")
        .unwrap_or_else(|_| DEFAULT_TRUNK_REGISTRY_DOMAIN.to_string());
    let url = format!("https://{}/api/v1/trunk-projects", domain);

    let response = reqwest::get(&url).await?;

    if response.status().is_success() {
        let projects: Vec<TrunkProjectMetadata> = response.json().await?;

        projects
            .into_iter()
            .find(|project| project.name == trunk_project)
            .ok_or_else(|| TrunkError::ProjectNotFound(trunk_project.to_owned()))
    } else {
        error!(
            "Failed to fetch metadata for trunk project {}: {}",
            trunk_project,
            response.status()
        );
        Err(TrunkError::NetworkFailure(
            response.error_for_status().unwrap_err(),
        ))
    }
}

// Get trunk project metadata for a specific version
pub async fn get_trunk_project_metadata_for_version(
    trunk_project_name: &str,
    version: Version<'_>,
) -> Result<TrunkProjectMetadata, TrunkError> {
    let domain = env::var("TRUNK_REGISTRY_DOMAIN")
        .unwrap_or_else(|_| DEFAULT_TRUNK_REGISTRY_DOMAIN.to_string());

    let url = match version {
        Version::TrunkProject(trunk_project_version) => format!(
            "https://{domain}/api/v1/trunk-projects/{trunk_project_name}/version/{trunk_project_version}"
        ),
        Version::Extension(_extension_version) => {
            format!("https://{domain}/api/v1/trunk-projects/{trunk_project_name}")
        }
    };

    let response = reqwest::get(&url).await?;

    if response.status().is_success().not() {
        error!(
            "Failed to fetch metadata for trunk project {trunk_project_name} version {version}: {}",
            response.status()
        );

        return Err(TrunkError::NetworkFailure(
            response.error_for_status().unwrap_err(),
        ));
    }

    let mut project_metadata: Vec<TrunkProjectMetadata> = response.json().await?;

    let trunk_project = match version {
        Version::TrunkProject(_) => {
            // There will only be one index here, so we can safely assume index 0
            match project_metadata.pop() {
                Some(project_metadata) => project_metadata,
                None => {
                    error!(
                        "Failed to fetch metadata for trunk project {trunk_project_name} with Trunk project version {version}"
                    );
                    return Err(TrunkError::TrunkProjectVersionNotFound(version.to_string()));
                }
            }
        }
        Version::Extension(extension_version) => {
            let trunk_project = project_metadata.into_iter().find(|metadata| {
                metadata.name == trunk_project_name
                    && metadata
                        .extensions
                        .iter()
                        .any(|ext| ext.version == extension_version)
            });

            match trunk_project {
                Some(project) => project,
                None => {
                    error!(
                        "Failed to fetch metadata for trunk project {trunk_project_name} with extension version {version}"
                    );
                    return Err(TrunkError::ExtensionVersionNotFound(version.to_string()));
                }
            }
        }
    };

    Ok(trunk_project)
}

// Check if extension name is in list of trunk project names
pub async fn extension_name_matches_trunk_project(extension_name: String) -> Result<bool, Action> {
    let trunk_project_names = match get_trunk_project_names().await {
        Ok(trunk_project_names) => trunk_project_names,
        Err(e) => {
            error!(
                "Failed to check if extension name and trunk project name match for {}: {:?}",
                extension_name, e
            );
            return Err(Action::requeue(Duration::from_secs(300)));
        }
    };
    Ok(trunk_project_names.contains(&extension_name))
}

// Find the trunk project name associated with a given extension
pub async fn get_trunk_project_for_extension(
    extension_name: String,
) -> Result<Option<String>, Action> {
    let trunk_projects = match get_trunk_projects().await {
        Ok(trunk_projects) => trunk_projects,
        Err(e) => {
            error!(
                "Failed to get trunk project name for extension {}: {:?}",
                extension_name, e
            );
            return Err(Action::requeue(Duration::from_secs(3)));
        }
    };
    // Check if the extension name matches a trunk project name
    if extension_name_matches_trunk_project(extension_name.clone()).await? {
        return Ok(Some(extension_name));
    }
    for trunk_project in trunk_projects {
        for extension in trunk_project.extensions {
            if extension.extension_name == extension_name {
                return Ok(Some(trunk_project.name));
            }
        }
    }
    Ok(None)
}

// Check if control file is absent for a given trunk project version
pub async fn is_control_file_absent(
    trunk_project: &str,
    version: Version<'_>,
) -> Result<bool, Action> {
    let project_metadata: TrunkProjectMetadata =
        match get_trunk_project_metadata_for_version(trunk_project, version).await {
            Ok(project_metadata) => project_metadata,
            Err(e) => {
                error!(
                    "Failed to get trunk project metadata for version {}: {:?}",
                    version, e
                );
                return Err(Action::requeue(Duration::from_secs(3)));
            }
        };
    // TODO(ianstanton) This assumes that there is only one extension in the project, but we need to handle the case
    //  where there are multiple extensions
    let control_file_absent = project_metadata
        .extensions
        .first()
        .unwrap()
        .control_file
        .absent;
    Ok(control_file_absent)
}

// Check if extension has loadable_library metadata for a given trunk project version and return the library name
pub async fn get_loadable_library_name(
    trunk_project: &str,
    version: Version<'_>,
    extension_name: &str,
) -> Result<Option<String>, Action> {
    let project_metadata: TrunkProjectMetadata =
        match get_trunk_project_metadata_for_version(trunk_project, version).await {
            Ok(project_metadata) => project_metadata,
            Err(e) => {
                error!(
                    "Failed to get trunk project metadata for version {}: {:?}",
                    version, e
                );
                return Err(Action::requeue(Duration::from_secs(3)));
            }
        };

    // Find the extension in the project metadata
    let extension_metadata = match project_metadata
        .extensions
        .iter()
        .find(|e| e.extension_name == extension_name)
    {
        Some(extension_metadata) => extension_metadata,
        None => {
            error!(
                "Failed to find extension {} in trunk project {} version {}",
                extension_name, trunk_project, version
            );
            return Err(Action::requeue(Duration::from_secs(3)));
        }
    };

    // Find the loadable library in the extension metadata
    let loadable_library_name = extension_metadata
        .loadable_libraries
        .as_ref()
        .and_then(|loadable_libraries| loadable_libraries.iter().find(|l| l.requires_restart))
        .map(|loadable_library| loadable_library.library_name.clone());
    Ok(loadable_library_name)
}

// Get trunk project description for a given trunk project version
pub async fn get_trunk_project_description(
    trunk_project: &str,
    version: Version<'_>,
) -> Result<Option<String>, Action> {
    let project_metadata: TrunkProjectMetadata =
        match get_trunk_project_metadata_for_version(trunk_project, version).await {
            Ok(project_metadata) => project_metadata,
            Err(e) => {
                error!(
                    "Failed to get trunk project metadata for version {}: {:?}",
                    version, e
                );
                return Err(Action::requeue(Duration::from_secs(3)));
            }
        };
    Ok(project_metadata.description)
}

// Get latest version of trunk project
pub async fn get_latest_trunk_project_version(trunk_project: &str) -> Result<String, Action> {
    match get_latest_trunk_project_metadata(trunk_project).await {
        Ok(project_metadata) => Ok(project_metadata.version),
        Err(e) => {
            error!(
                "Failed to get trunk project metadata for {}: {:?}",
                trunk_project, e
            );

            Err(Action::requeue(Duration::from_secs(3)))
        }
    }
}

// Check if string version is semver
pub fn is_semver(version: &str) -> bool {
    semver::Version::parse(version).is_ok()
}

// Convert to semver if not already
pub fn convert_to_semver(version: &str) -> Cow<'_, str> {
    let mut version = Cow::Borrowed(version);
    if !is_semver(&version) {
        version.to_mut().push_str(".0");
    }
    version
}

// Define error type
#[derive(Debug, thiserror::Error)]
pub enum TrunkError {
    #[error("Trunk project with name '{0}' not found")]
    ProjectNotFound(String),
    #[error("Failed to fetch metadata from trunk: {0}")]
    NetworkFailure(#[from] reqwest::Error),
    #[error("Failed to apply trunk configmap")]
    ConfigMapApplyError,
    #[error("Extension with version '{0}' not found")]
    ExtensionVersionNotFound(String),
    #[error("Trunk project with version '{0}' not found")]
    TrunkProjectVersionNotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_trunk_projects() {
        let result = get_trunk_projects().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_trunk_project_names() {
        let result = get_trunk_project_names().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_latest_trunk_project_version() {
        let result = get_latest_trunk_project_metadata("pgmq").await;
        assert!(result.is_ok());

        let project = result.unwrap();
        assert!(project.version == "1.3.3");
        assert!(project.name == "pgmq");
    }

    #[tokio::test]
    async fn test_get_trunk_project_metadata_for_version() {
        let trunk_project = "auto_explain";
        let version = "15.3.0";
        let result = get_trunk_project_metadata_for_version(
            trunk_project,
            super::Version::TrunkProject(version),
        )
        .await;
        assert!(result.is_ok());

        let trunk_project = result.unwrap();

        assert!(trunk_project.version == "15.3.0");
        assert!(trunk_project.name == "auto_explain");
    }

    #[tokio::test]
    async fn test_get_trunk_project_metadata_for_extension_version() {
        // Find metadata on citext with extension version 1.6
        let result =
            get_trunk_project_metadata_for_version("citext", super::Version::Extension("1.6"))
                .await;

        let trunk_project = result.unwrap();

        assert!(trunk_project.version == "1.6.0");
        assert!(trunk_project.name == "citext");

        // Ensure that if we tried to find citext through an extension version of 1.6.0 (which is incorrect),
        // we'd find no results
        assert!(get_trunk_project_metadata_for_version(
            "citext",
            super::Version::Extension("1.6.0"),
        )
        .await
        .is_err())
    }

    #[tokio::test]
    async fn test_extension_name_matches_trunk_project() {
        let extension_name = "auto_explain".to_string();
        let result = extension_name_matches_trunk_project(extension_name).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        let extension_name = "pgml".to_string();
        let result = extension_name_matches_trunk_project(extension_name).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());

        let extension_name = "vector".to_string();
        let result = extension_name_matches_trunk_project(extension_name).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_get_trunk_project_for_extension() {
        let extension_name = "auto_explain".to_string();
        let result = get_trunk_project_for_extension(extension_name).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("auto_explain".to_string()));

        let extension_name = "pgml".to_string();
        let result = get_trunk_project_for_extension(extension_name).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("postgresml".to_string()));

        let extension_name = "vector".to_string();
        let result = get_trunk_project_for_extension(extension_name).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("pgvector".to_string()));

        let extension_name = "columnar".to_string();
        let result = get_trunk_project_for_extension(extension_name).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("hydra_columnar".to_string()));
    }

    #[tokio::test]
    async fn test_is_control_file_absent() {
        let trunk_project = "auto_explain";
        let version = "15.3.0";
        let result = is_control_file_absent(trunk_project, Version::TrunkProject(version)).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_get_loadable_library_name() {
        let trunk_project = "auto_explain";
        let version = "15.3.0";
        let extension_name = "auto_explain";
        let result = get_loadable_library_name(
            trunk_project,
            Version::TrunkProject(version),
            extension_name,
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("auto_explain".to_string()));
    }

    #[tokio::test]
    async fn test_get_trunk_project_description() {
        let trunk_project = "auto_explain";
        let version = "15.3.0";
        let result =
            get_trunk_project_description(trunk_project, Version::TrunkProject(version)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("The auto_explain module provides a means for logging execution plans of slow statements automatically, without having to run EXPLAIN by hand.".to_string()));
    }

    #[test]
    fn test_is_semver() {
        let version = "1.2.3";
        let result = is_semver(version);
        assert!(result);

        let version = "1.2";
        let result = is_semver(version);
        assert!(!result);
    }

    #[test]
    fn test_convert_to_semver() {
        let version = "1.2.3";
        let result = convert_to_semver(version);
        assert_eq!(result, "1.2.3");

        let version = "1.2";
        let result = convert_to_semver(version);
        assert_eq!(result, "1.2.0".to_string());
    }
}
