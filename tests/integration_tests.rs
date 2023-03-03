use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions
use rand::Rng;
use std::process::Command; // Run programs

const CARGO_BIN: &str = "trunk";

#[test]
fn help() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;

    cmd.arg("--help");
    cmd.assert().stdout(predicate::str::contains("Usage: "));

    Ok(())
}

#[test]
fn install() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("install");
    cmd.assert()
        .stdout(predicate::str::contains("not implemented"));
    Ok(())
}

#[test]
fn pgmq() -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = rand::thread_rng();
    let output_dir = format!("/tmp/pgmq_test_{}", rng.gen_range(0..1000000));

    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    // TODO replace by a hello-world pgx extension instead of assuming a local extension is present
    cmd.arg("build");
    cmd.arg("--path");
    cmd.arg("/Users/steven/CLionProjects/coredb/extensions/pgmq");
    cmd.arg("--output-path");
    cmd.arg(output_dir.clone());
    cmd.assert().code(0);
    assert!(std::path::Path::new(format!("{output_dir}/result.tar").as_str()).exists());
    // delete the temporary file
    std::fs::remove_dir_all(output_dir)?;

    Ok(())
}
