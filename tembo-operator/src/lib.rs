/// Expose all controller components used by main
pub mod controller;
pub use crate::controller::*;
pub mod apis;

pub mod app_service;
pub mod configmap;
pub mod extensions;
pub mod postgres_exporter;
/// Log and trace integrations
pub mod telemetry;

mod exec;
/// Metrics
mod metrics;
pub use metrics::Metrics;
mod config;
pub mod defaults;
pub mod errors;
pub mod prometheus;

pub mod cloudnativepg;
mod deployment_postgres_exporter;
#[cfg(test)]
pub mod fixtures;
pub mod heartbeat;
pub mod ingress;
pub mod traefik;
pub use traefik::ingress_route_crd;
mod certmanager;
mod network_policies;
pub mod postgres_certificates;
pub mod psql;
mod rbac;
mod secret;
mod service;
pub mod snapshots;
mod trunk;

pub const RESTARTED_AT: &str = "kubectl.kubernetes.io/restartedAt";

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("An error occurred in kube-exec: {0}")]
    KubeExecError(String),

    #[error("SerializationError: {0}")]
    SerializationError(#[source] serde_json::Error),

    #[error("SerializationError: {0}")]
    YamlSerializationError(#[source] serde_yaml::Error),

    #[error("Kube Error: {0}")]
    KubeError(#[from] kube::Error),

    #[error("Finalizer Error: {0}")]
    // NB: awkward type because finalizer::Error embeds the reconciler error (which is this)
    // so boxing this error to break cycles
    FinalizerError(#[source] Box<kube::runtime::finalizer::Error<Error>>),

    #[error("Pod Error: {0}")]
    PodError(String),

    #[error("Missing Secret Error: {0}")]
    MissingSecretError(String),

    #[error("Invalid Data: {0}")]
    InvalidErr(String),
}
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn metric_label(&self) -> String {
        format!("{self:?}").to_lowercase()
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(err: serde_yaml::Error) -> Self {
        Error::YamlSerializationError(err)
    }
}
