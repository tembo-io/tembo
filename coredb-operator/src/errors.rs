use thiserror::Error;

#[derive(Error, Debug)]
pub enum OperatorError {
    #[error("An IngressRouteTCP did not have a name")]
    IngressRouteTCPNameError,

    #[error("An IngressRouteTCP failed to update")]
    IngressRouteTCPUpdateError,

    #[error("An IngressRouteTCP failed to create")]
    IngressRouteTCPCreateError,

    #[error("KubeError: {0}")]
    KubeError(#[from] kube::Error),
}
