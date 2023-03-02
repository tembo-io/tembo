//! Custom errors types for pgui-api
use thiserror::Error;
use url::ParseError;

#[derive(Error, Debug)]
pub enum PgUIAPIError {
    /// a url parsing error
    #[error("url parsing error {0}")]
    UrlParsingError(#[from] ParseError),

    /// a database error
    #[error("database error {0}")]
    DatabaseError(#[from] sqlx::Error),
}
