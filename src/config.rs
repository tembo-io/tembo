use std::env;

#[derive(Debug, Clone)]
pub struct S3Config {
    pub bucket_name: String,
    pub region: String,
    pub aws_access_key: String,
    pub aws_secret_key: String,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            bucket_name: from_env_default("BUCKET_NAME", "trunk-registry"),
            region: from_env_default("REGION", "us-east-1"),
            aws_access_key: from_env_default("AWS_ACCESS_KEY_ID", ""),
            aws_secret_key: from_env_default("AWS_SECRET_ACCESS_KEY", ""),
        }
    }
}

/// source a variable from environment - use default if not exists
fn from_env_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}
