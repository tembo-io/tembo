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
fn build_extension() -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = rand::thread_rng();
    let output_dir = format!("/tmp/pgmq_test_{}", rng.gen_range(0..1000000));

    // Construct a path relative to the current file's directory
    let mut extension_path = std::path::PathBuf::from(file!());
    extension_path.pop(); // Remove the file name from the path
    extension_path.push("test_extension");

    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("build");
    cmd.arg("--path");
    cmd.arg(extension_path.as_os_str());
    cmd.arg("--output-path");
    cmd.arg(output_dir.clone());
    cmd.assert().code(0);
    assert!(std::path::Path::new(format!("{output_dir}/result.tar").as_str()).exists());
    // delete the temporary file
    std::fs::remove_dir_all(output_dir)?;

    Ok(())
}
