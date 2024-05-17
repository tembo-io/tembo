use crate::errors::PlatformError;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions;
use sqlx::{Pool, Postgres};
use url::{ParseError, Url};

pub async fn connect(url: &str, max_connections: u32) -> Result<Pool<Postgres>, PlatformError> {
    let options = conn_options(url)?;
    let pgp = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(10))
        .max_connections(max_connections)
        .connect_with(options)
        .await?;
    Ok(pgp)
}

// Configure connection options
pub fn conn_options(url: &str) -> Result<PgConnectOptions, PlatformError> {
    // Parse url
    let parsed = Url::parse(url)?;
    let options = PgConnectOptions::new()
        .host(parsed.host_str().ok_or(ParseError::EmptyHost)?)
        .port(parsed.port().ok_or(ParseError::InvalidPort)?)
        .username(parsed.username())
        .password(parsed.password().ok_or(ParseError::IdnaError)?)
        .log_statements(log::LevelFilter::Debug);
    Ok(options)
}
