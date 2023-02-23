//! Custom errors types for webserver
use thiserror::Error;
use url::ParseError;

#[derive(Error, Debug)]
pub enum WebserverError {
    /// a url parsing error
    #[error("url parsing error {0}")]
    UrlParsingError(#[from] ParseError),

    /// a database error
    #[error("database error {0}")]
    DatabaseError(#[from] sqlx::Error),
}
