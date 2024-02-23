use aws_sdk_cloudformation::Error as CFError;
use kube;
use pgmq::errors::PgmqError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConductorError {
    /// a json parsing error
    #[error("json parsing error {0}")]
    JsonParsingError(#[from] serde_json::error::Error),

    /// a kube error
    #[error("kube error {0}")]
    KubeError(#[from] kube::Error),

    // No status reported
    #[error("no status reported")]
    NoStatusReported,

    #[error("Error parsing event ID {0}")]
    EventIDParsing(String),

    #[error("Error formatting event ID")]
    EventIDFormat,

    #[error("Error using queue {0}")]
    PgmqError(#[from] PgmqError),

    /// a aws error
    #[error("aws sdk error {0}")]
    AwsError(#[from] Box<CFError>),

    // No outputs found for the stack
    #[error("no outputs found for the stack")]
    NoOutputsFound,

    #[error("Didn't find Postgres connection information")]
    PostgresConnectionInfoNotFound,

    #[error("Failed to parse postgres connection information")]
    ParsingPostgresConnectionError,

    #[error("Secret data not found for: {0}")]
    SecretDataNotFound(String),

    #[error("Name or Namespace was not for for: {0}")]
    NameOrNamespaceNotFound(String),

    #[error("CoreDB Restore spec not found for: {0}")]
    CoreDBRestoreSpecNotFound(String),
}
