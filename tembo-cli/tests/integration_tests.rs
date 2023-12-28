use assert_cmd::prelude::*; // Add methods on commands
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres_openssl::MakeTlsConnector;
use predicates::prelude::*; // Used for writing assertions
use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use tokio_postgres::config::SslMode;
use tokio_postgres::Config;

const CARGO_BIN: &str = "tembo";

#[test]
fn help() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;

    cmd.arg("--help");
    cmd.assert().stdout(predicate::str::contains("Usage: "));

    Ok(())
}

#[tokio::test]
async fn minimal() -> Result<(), Box<dyn Error>> {
    let root_dir = env!("CARGO_MANIFEST_DIR");
    let test_dir = PathBuf::from(root_dir)
        .join("tests")
        .join("tomls")
        .join("minimal");

    env::set_current_dir(&test_dir)?;

    // tembo init
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("init");
    cmd.assert().success();

    // tembo context set --name local
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("context");
    cmd.arg("set");
    cmd.arg("--name");
    cmd.arg("local");
    cmd.assert().success();

    // tembo apply
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("--verbose");
    cmd.arg("apply");
    cmd.assert().success();

    // check can connect
    assert_can_connect().await?;

    // tembo delete
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    cmd.assert().success();

    // check can't connect
    assert!(assert_can_connect().await.is_err());

    Ok(())
}

#[tokio::test]
async fn data_warehouse() -> Result<(), Box<dyn Error>> {
    let root_dir = env!("CARGO_MANIFEST_DIR");
    let test_dir = PathBuf::from(root_dir)
        .join("tests")
        .join("tomls")
        .join("data-warehouse");

    env::set_current_dir(&test_dir)?;

    // tembo init
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("init");
    cmd.assert().success();

    // tembo context set --name local
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("context");
    cmd.arg("set");
    cmd.arg("--name");
    cmd.arg("local");
    cmd.assert().success();

    // tembo apply
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("--verbose");
    cmd.arg("apply");
    cmd.assert().success();

    // check can connect
    assert_can_connect().await?;

    // check extensions includes postgres_fdw in the output
    // connecting to postgres and running the command

    // tembo delete
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    cmd.assert().success();

    // check can't connect
    assert!(assert_can_connect().await.is_err());

    Ok(())
}

async fn get_output_from_sql(sql: String) -> Result<String, Box<dyn Error>> {
    let mut config = Config::new();
    config.host("localhost");
    config.user("postgres");
    config.password("postgres");
    config.dbname("postgres");
    config.port(5432);
    config.ssl_mode(SslMode::Require); // Set SSL mode to "require"

    let mut builder =
        SslConnector::builder(SslMethod::tls()).expect("unable to create sslconnector builder");
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
    let rows = client.query(&sql, &[]).await?;
    let total_output = rows
        .iter()
        .map(|row| {
            let mut output = String::new();
            for (i, column) in row.columns().iter().enumerate() {
                match column.type_().name() {
                    "int4" => {
                        // If the column is an integer, use get::<_, i32>
                        let value: Option<i32> = row.get(i);
                        output.push_str(&format!("{}: {:?} ", column.name(), value));
                    }
                    _ => {
                        // Fallback for other types, adjust as needed
                        let value = row.get::<_, Option<&str>>(i);
                        output.push_str(&format!("{}: {:?} ", column.name(), value));
                    }
                }
            }
            output
        })
        .collect::<Vec<String>>()
        .join("\n");

    Ok(total_output)
}

async fn assert_can_connect() -> Result<(), Box<dyn Error>> {
    let result = get_output_from_sql("SELECT 1".to_string()).await?;
    assert!(result.contains('1'), "Query did not return 1");
    Ok(())
}
