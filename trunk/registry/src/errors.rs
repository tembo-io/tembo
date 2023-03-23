//! Custom errors types for extension registry
use actix_web::error;
use thiserror::Error;
use url::ParseError;

// Use default implementation for `error_response()` method
impl error::ResponseError for ExtensionRegistryError {}

#[derive(Error, Debug)]
pub enum ExtensionRegistryError {
    /// a url parsing error
    #[error("url parsing error {0}")]
    UrlParsingError(#[from] ParseError),

    /// a database error
    #[error("database error {0}")]
    DatabaseError(#[from] sqlx::Error),

    /// a response error
    #[error("response error")]
    ResponseError(),

    /// a payload error
    #[error("payload error")]
    PayloadError(#[from] error::PayloadError),

    /// a bad request error
    #[error("bad request error")]
    ErrorBadRequest(#[from] error::Error),

    /// a serde json error
    #[error("serde json error")]
    SerdeJsonError(#[from] serde_json::Error),
}
