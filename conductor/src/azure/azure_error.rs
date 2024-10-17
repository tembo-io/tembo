use azure_core::Error as AzureSDKError;
use reqwest::Error as ReqwestError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AzureError {
    #[error("Error with Azure SDK {0}")]
    AzureSDKError(#[from] AzureSDKError),

    #[error("Error with Azure REST API {0}")]
    AzureRestAPIError(#[from] ReqwestError),
}
