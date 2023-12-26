use std::env;
use std::error::Error;
use std::path::PathBuf;
use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions
use std::process::Command;
use tokio_postgres::{Client, NoTls, Config};
use tokio_postgres::config::SslMode;
use postgres_openssl::MakeTlsConnector;
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};

const CARGO_BIN: &str = "tembo";

#[test]
fn help() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;

    cmd.arg("--help");
    cmd.assert().stdout(predicate::str::contains("Usage: "));

    Ok(())
}

#[tokio::test]
async fn minimal() ->  Result<(), Box<dyn Error>> {

    // Get the root directory of the crate
    let root_dir = env!("CARGO_MANIFEST_DIR");
    let test_dir = PathBuf::from(root_dir)
        .join("tests")  // Adjust the path to the tests directory
        .join("tomls")
        .join("minimal");

    // Change the current working directory
    env::set_current_dir(&test_dir)?;

    // Execute the command
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("apply");
    cmd.assert().success();


    // PostgreSQL connection string
    let mut config = Config::new();
    config.host("localhost");
    config.user("postgres");
    config.password("postgres");
    config.dbname("postgres");
    config.port(5432);
    config.ssl_mode(SslMode::Require); // Set SSL mode to "require"

    let mut builder = SslConnector::builder(SslMethod::tls()).expect("unable to create sslconnector builder");
    builder.set_verify(SslVerifyMode::NONE);
    let connector = MakeTlsConnector::new(builder.build());

    // Connect to the PostgreSQL database
    let (client, connection) = config.connect(connector).await?;

    // The connection object performs the actual communication with the database,
    // so spawn it off to run on its own.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    // Execute a simple query
    let rows = client.query("SELECT 1", &[]).await?;

    // Check that the query returned exactly one row with one column
    assert_eq!(rows.len(), 1);
    let value: i32 = rows[0].get(0);
    assert_eq!(value, 1, "Query did not return 1");

    Ok(())
}

