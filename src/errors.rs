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

    #[error("response error")]
    ResponseError(),
}
