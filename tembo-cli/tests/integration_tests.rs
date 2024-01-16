use assert_cmd::prelude::*; // Add methods on commands

use colorful::core::StrMarker;
use predicates::prelude::*;
use std::error::Error;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::{env, io};

use std::io::Write;

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
    assert_can_connect("minimal".to_str()).await?;

    // tembo delete
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    cmd.assert().success();

    // check can't connect
    assert!(assert_can_connect("minimal".to_str()).await.is_err());

    Ok(())
}

#[tokio::test]
async fn data_warehouse() -> Result<(), Box<dyn Error>> {
    let instance_name = "data-warehouse";

    let root_dir = env!("CARGO_MANIFEST_DIR");
    let test_dir = PathBuf::from(root_dir)
        .join("tests")
        .join("tomls")
        .join(instance_name);

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
    assert_can_connect(instance_name.to_string()).await?;

    // check extensions includes postgres_fdw in the output
    // connecting to postgres and running the command
    let result = get_output_from_sql(
        instance_name.to_string(),
        "SELECT * FROM pg_extension WHERE extname = 'clerk_fdw'".to_string(),
    );
    assert!(result.await?.contains("clerk_fdw"));

    // tembo delete
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    cmd.assert().success();

    // check can't connect
    assert!(assert_can_connect(instance_name.to_string()).await.is_err());

    Ok(())
}

#[tokio::test]
async fn multiple_instances() -> Result<(), Box<dyn Error>> {
    let root_dir = env!("CARGO_MANIFEST_DIR");
    let test_dir = PathBuf::from(root_dir)
        .join("tests")
        .join("tomls")
        .join("multiple-instances");

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
    assert_can_connect("defaults_instance".to_str()).await?;
    assert_can_connect("mobile_instance".to_str()).await?;

    // tembo delete
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    cmd.assert().success();

    // check can't connect
    assert!(assert_can_connect("defaults_instance".to_str())
        .await
        .is_err());
    assert!(assert_can_connect("mobile_instance".to_str())
        .await
        .is_err());

    Ok(())
}

async fn get_output_from_sql(instance_name: String, sql: String) -> Result<String, Box<dyn Error>> {
    // Command to execute psql
    let mut child = Command::new("psql")
        .arg("-h") // Hostname
        .arg(format!("{}.local.tembo.io", instance_name))
        .arg("-U") // User
        .arg("postgres")
        .arg("-d") // Database name
        .arg("postgres")
        .arg("-p") // Port
        .arg("5432")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Writing SQL command to psql's stdin
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(sql.as_bytes())?;
    } else {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "Failed to write to stdin",
        )));
    }

    // Capturing the output
    let output = child.wait_with_output()?;

    // Check if the command was successful
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Box::new(io::Error::new(io::ErrorKind::Other, err)));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn assert_can_connect(instance_name: String) -> Result<(), Box<dyn Error>> {
    let result = get_output_from_sql(instance_name, "SELECT 1".to_string()).await?;
    assert!(result.contains('1'), "Query did not return 1");
    Ok(())
}
