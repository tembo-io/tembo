use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, Pool, Postgres};

pub mod config;
pub mod download;
pub mod errors;
pub mod publish;
pub mod routes;
pub mod uploader;
pub mod views;

use log::LevelFilter;
use url::{ParseError, Url};


// Connect to postgresql server
pub async fn connect(url: &str) -> Result<Pool<Postgres>, errors::ExtensionRegistryError> {
    let options = conn_options(url)?;
    let pgp = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(10))
        .max_connections(5)
        .connect_with(options)
        .await?;
    Ok(pgp)
}

// Configure connection options
pub fn conn_options(url: &str) -> Result<PgConnectOptions, errors::ExtensionRegistryError> {
    // Parse url
    let parsed = Url::parse(url)?;
    let mut path_segments = parsed.path_segments().ok_or("cannot be base").unwrap();
    let mut options = PgConnectOptions::new()
        .host(parsed.host_str().ok_or(ParseError::EmptyHost)?)
        .port(parsed.port().ok_or(ParseError::InvalidPort)?)
        .username(parsed.username())
        .password(parsed.password().ok_or(ParseError::IdnaError)?)
        .database(path_segments.next().unwrap());
    options.log_statements(LevelFilter::Debug);
    Ok(options)
}
