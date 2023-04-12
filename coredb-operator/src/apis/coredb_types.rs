use crate::extensions::Extension;
use k8s_openapi::{api::core::v1::ResourceRequirements, apimachinery::pkg::api::resource::Quantity};

use crate::defaults;
use kube::CustomResource;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Generate the Kubernetes wrapper struct `CoreDB` from our Spec and Status struct
///
/// This provides a hook for generating the CRD yaml (in crdgen.rs)
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[cfg_attr(test, derive(Default))]
#[kube(kind = "CoreDB", group = "coredb.io", version = "v1alpha1", namespaced)]
#[kube(status = "CoreDBStatus", shortname = "cdb")]
#[allow(non_snake_case)]
pub struct CoreDBSpec {
    #[serde(default = "defaults::default_replicas")]
    pub replicas: i32,
    #[serde(default = "defaults::default_resources")]
    pub resources: ResourceRequirements,
    #[serde(default = "defaults::default_storage")]
    pub storage: Quantity,
    #[serde(default = "defaults::default_sharedir_storage")]
    pub sharedirStorage: Quantity,
    #[serde(default = "defaults::default_pkglibdir_storage")]
    pub pkglibdirStorage: Quantity,
    #[serde(default = "defaults::default_postgres_exporter_enabled")]
    pub postgresExporterEnabled: bool,
    #[serde(default = "defaults::default_image")]
    pub image: String,
    #[serde(default = "defaults::default_postgres_exporter_image")]
    pub postgresExporterImage: String,
    #[serde(default = "defaults::default_port")]
    pub port: i32,
    #[serde(default = "defaults::default_uid")]
    pub uid: i32,
    #[serde(default = "defaults::default_extensions")]
    pub extensions: Vec<Extension>,
    #[serde(default = "defaults::default_stop")]
    pub stop: bool,
}

/// The status object of `CoreDB`
#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[allow(non_snake_case)]
pub struct CoreDBStatus {
    pub running: bool,
    #[serde(default = "defaults::default_extensions_updating")]
    pub extensionsUpdating: bool,
    #[serde(default = "defaults::default_extensions")]
    pub extensions: Vec<Extension>,
    #[serde(default = "defaults::default_storage")]
    pub storage: Quantity,
    #[serde(default = "defaults::default_sharedir_storage")]
    pub sharedirStorage: Quantity,
    #[serde(default = "defaults::default_pkglibdir_storage")]
    pub pkglibdirStorage: Quantity,
}
