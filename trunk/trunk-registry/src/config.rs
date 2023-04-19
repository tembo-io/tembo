use actix_web::http::header::HeaderValue;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bucket_name: String,
    pub region: Option<String>,
    pub aws_access_key: String,
    pub aws_secret_key: String,
    pub auth_token: HeaderValue,
}

// TODO(ianstanton) Fix load from .env
impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: from_env_default(
                "DATABASE_URL",
                "postgres://postgres@localhost/trunk_registry",
            ),
            bucket_name: from_env_default("S3_BUCKET", "trunk-registry"),
            region: Some(from_env_default("S3_REGION", "")),
            aws_access_key: from_env_default("AWS_ACCESS_KEY", ""),
            aws_secret_key: from_env_default("AWS_SECRET_KEY", ""),
            auth_token: from_env_default("AUTH_TOKEN", "").parse().unwrap(),
        }
    }
}

/// source a variable from environment - use default if not exists
fn from_env_default(key: &str, default: &str) -> String {
    dotenv::var(key).unwrap_or_else(|_| env::var(key).unwrap_or_else(|_| default.to_owned()))
}
