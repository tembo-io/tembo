// All public facing errors are ErrorResponse
// All internal errors map to PlatformError
// PlatformError maps to ErrorResponse

use serde::ser::SerializeMap;
use serde::Serialize;
use sqlx::error::Error as DbError;
use std::str::Utf8Error;
use thiserror::Error;

use actix_web::{http::StatusCode, web::JsonConfig, HttpResponse, ResponseError};
use serde_json;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database error: {0}")]
    DbError(#[from] DbError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("Expected information missing")]
    MissingInfoError,
    #[error("Unhandled database error")]
    Unhandled,
}

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("{0}")]
    TokenError(String),
    #[error("{0}")]
    ParsingError(String),
    #[error("{0}")]
    NotAuthorized(String),
    #[error("{0}")]
    Forbidden(String),
}

#[derive(Error, Debug)]
pub enum PlatformError {
    #[error("{0}")]
    AuthError(#[from] AuthError),
    #[error("{0}")]
    Conflict(String),
    #[error("NotFoundError: {0}")]
    NotFoundError(String),
    #[error("Database error: {0}")]
    DbError(#[from] DbError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Url error: {0}")]
    UrlError(#[from] url::ParseError),
    #[error("db error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Invalid Type {0}")]
    TypeError(String),
    #[error("Serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("ValueError: {0}")]
    ValueError(String),
    #[error("FloatError: {0}")]
    FloatError(#[from] std::num::ParseFloatError),
    #[error("Byte error: {0}")]
    ByteError(#[from] Utf8Error),
    #[error("Bad request error: {0}")]
    InvalidQuery(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Parse int error: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error("HTTP error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("S3 error: {0}")]
    S3Error(String),
}

// PUBLIC FACING ERROR RESPOSNES
#[derive(Error, Debug)]
pub enum ErrorResponse {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    NotAuthorized(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    InternalServerError(String),
}

impl Serialize for ErrorResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let variant_str = format!("{}", self);
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("error", &variant_str)?;
        map.end()
    }
}

impl ResponseError for PlatformError {
    fn error_response(&self) -> HttpResponse {
        let resp = match self {
            PlatformError::AuthError(_) => ErrorResponse::NotAuthorized(self.to_string()),
            PlatformError::Conflict(_) => ErrorResponse::Conflict(self.to_string()),
            PlatformError::NotFoundError(_) => ErrorResponse::NotFound(self.to_string()),
            PlatformError::InvalidQuery(_) => ErrorResponse::BadRequest(self.to_string()),
            PlatformError::ValueError(_) => ErrorResponse::BadRequest(self.to_string()),
            _ => ErrorResponse::InternalServerError(
                "Internal Server Error. Please contact support".to_string(),
            ),
        };
        HttpResponse::build(self.status_code()).json(resp)
    }
    fn status_code(&self) -> StatusCode {
        match *self {
            PlatformError::AuthError(AuthError::Forbidden(_)) => StatusCode::FORBIDDEN,
            PlatformError::AuthError(_) => StatusCode::UNAUTHORIZED,
            PlatformError::Conflict(_) => StatusCode::CONFLICT,
            PlatformError::NotFoundError(_) => StatusCode::NOT_FOUND,
            PlatformError::InvalidQuery(_) => StatusCode::BAD_REQUEST,
            PlatformError::ValueError(_) => StatusCode::BAD_REQUEST,
            _ => {
                log::error!("Internal Server Error: {:?}", self);
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

pub fn make_json_config() -> JsonConfig {
    use actix_web::error::InternalError;

    JsonConfig::default().error_handler(|error, _request| {
        #[derive(Serialize)]
        struct ErrorBody<T: Serialize> {
            error: T,
        }

        let error_msg = error.to_string();
        let error_body = ErrorBody { error: error_msg };
        let response = HttpResponse::BadRequest().json(error_body);

        InternalError::from_response(error, response).into()
    })
}
