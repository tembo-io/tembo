use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions
use std::process::Command; // Run programs

const CARGO_BIN: &str = "tembo";

#[test]
fn help() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;

    cmd.arg("--help");
    cmd.assert().stdout(predicate::str::contains("Usage: "));

    Ok(())
}

/*
#[test]
fn init() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("init");

    cmd.assert()
        .stdout(predicate::str::contains("Tembo home directory created"));
    cmd.assert()
        .stdout(predicate::str::contains("Tembo context file created"));
    cmd.assert()
        .stdout(predicate::str::contains("Tembo config file created"));
    cmd.assert().stdout(predicate::str::contains(
        "Tembo migrations directory created",
    ));

    Ok(())
}
 */
