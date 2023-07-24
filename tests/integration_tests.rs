use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions
use std::process::Command; // Run programs

const CARGO_BIN: &str = "tembo-cli";

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
        .arg("db")
        .arg("--dry-run")
        .arg("sample-db");
    cmd.assert().stdout(predicate::str::contains("kind: Tembo"));

    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("create")
        .arg("--dry-run")
        .arg("db")
        .arg("sample-db");
    cmd.assert().stdout(predicate::str::contains("kind: Tembo"));

    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("create")
        .arg("db")
        .arg("sample-db")
        .arg("--dry-run");
    cmd.assert().stdout(predicate::str::contains("kind: Tembo"));

    Ok(())
}
