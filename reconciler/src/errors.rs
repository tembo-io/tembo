use kube;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReconcilerError {
    /// a json parsing error
    #[error("json parsing error {0}")]
    JsonParsingError(#[from] serde_json::error::Error),

    /// a kube error
    #[error("kube error {0}")]
    KubeError(#[from] kube::Error),

    // No status reported
    #[error("no status reported")]
    NoStatusReported,
}
