use thiserror::Error;

#[derive(Error, Debug)]
pub enum OperatorError {
    #[error("An IngressRouteTCP did not have a name")]
    IngressRouteTCPName,

    #[error("An IngressRouteTCP failed to update")]
    IngressRouteTCPUpdate,

    #[error("An IngressRouteTCP failed to create")]
    IngressRouteTCPCreate,

    #[error("KubeErr: {0}")]
    KubeErr(#[from] kube::Error),
}
