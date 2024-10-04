use std::str::Utf8Error;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum OperatorError {
    #[error("An IngressRouteTCP did not have a name")]
    IngressRouteTCPName,

    #[error("An IngressRoute failed to Create, Update, or Delete")]
    IngressRouteError,

    #[error("An IngressRouteTCP failed to Create, Update, or Delete")]
    IngressRouteTcpError,

    #[error("Failed to create, update, or delete NetworkPolicy: {0}")]
    NetworkPolicyError(String),

    #[error("Failed to create, update, or delete Service: {0}")]
    ServiceError(String),

    #[error("KubeErr: {0}")]
    KubeErr(#[from] kube::Error),

    #[error("ValueError: {0}")]
    ValueError(#[from] ValueError),
}

#[derive(Error, Debug)]
pub enum ValueError {
    #[error("Invalid value: {0}")]
    Invalid(String),
    #[error("Byte error: {0}")]
    ByteError(#[from] Utf8Error),
    #[error("FloatError: {0}")]
    FloatError(#[from] std::num::ParseFloatError),
    #[error("DateTime Parse Error: {0}")]
    ChronoParseError(#[from] chrono::format::ParseError),
}
