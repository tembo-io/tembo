use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions
use std::process::Command; // Run programs

const CARGO_BIN: &str = "coredb-cli";

#[test]
fn help() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;

    cmd.arg("--help");
    cmd.assert().stdout(predicate::str::contains("Usage: "));

    Ok(())
}

#[test]
fn create_dry_run() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("create")
        .arg("--resource-type")
        .arg("db")
        .arg("--dry-run")
        .arg("--name")
        .arg("sample-db");
    cmd.assert()
        .stdout(predicate::str::contains("kind: CoreDB"));

    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("create")
        .arg("--dry-run")
        .arg("--resource-type")
        .arg("db")
        .arg("--name")
        .arg("sample-db");
    cmd.assert()
        .stdout(predicate::str::contains("kind: CoreDB"));

    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("create")
        .arg("--resource-type")
        .arg("db")
        .arg("--name")
        .arg("sample-db")
        .arg("--dry-run");
    cmd.assert()
        .stdout(predicate::str::contains("kind: CoreDB"));

    Ok(())
}
