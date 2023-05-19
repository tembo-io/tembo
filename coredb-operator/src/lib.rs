/// Expose all controller components used by main
pub mod controller;
pub use crate::controller::*;
pub mod apis;

pub mod postgres_exporter;
/// Log and trace integrations
pub mod telemetry;

mod exec;
/// Metrics
mod metrics;
pub use metrics::Metrics;
mod config;
mod cronjob;
pub mod defaults;
mod extensions;
#[cfg(test)] pub mod fixtures;
mod psql;
mod rbac;
mod secret;
mod service;
mod statefulset;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("An error occurred in kube-exec: {0}")]
    KubeExecError(String),

    #[error("SerializationError: {0}")]
    SerializationError(#[source] serde_json::Error),

    #[error("Kube Error: {0}")]
    KubeError(#[from] kube::Error),

    #[error("Finalizer Error: {0}")]
    // NB: awkward type because finalizer::Error embeds the reconciler error (which is this)
    // so boxing this error to break cycles
    FinalizerError(#[source] Box<kube::runtime::finalizer::Error<Error>>),

    #[error("Pod Error: {0}")]
    PodError(String),
}
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn metric_label(&self) -> String {
        format!("{self:?}").to_lowercase()
    }
}
