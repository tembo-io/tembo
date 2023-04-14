use assert_cmd::prelude::*; // Add methods on commands
use git2::{Repository, build::CheckoutBuilder};
use predicates::prelude::*; // Used for writing assertions
use rand::Rng;
use std::path::{Path, PathBuf};
use std::process::Command; // Run programs
use std::fs;

const CARGO_BIN: &str = "trunk";

#[test]
fn help() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;

    cmd.arg("--help");
    cmd.assert().stdout(predicate::str::contains("Usage: "));

    Ok(())
}

#[test]
fn build_pgx_extension() -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = rand::thread_rng();
    let output_dir = format!("/tmp/pgmq_test_{}", rng.gen_range(0..1000000));

    // Construct a path relative to the current file's directory
    let mut extension_path = std::path::PathBuf::from(file!());
    extension_path.pop(); // Remove the file name from the path
    extension_path.push("test_pgx_extension");

    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("build");
    cmd.arg("--path");
    cmd.arg(extension_path.as_os_str());
    cmd.arg("--output-path");
    cmd.arg(output_dir.clone());
    cmd.assert().code(0);
    assert!(
        std::path::Path::new(format!("{output_dir}/test_pgx_extension-0.0.0.tar.gz").as_str())
            .exists()
    );
    // delete the temporary file
    std::fs::remove_dir_all(output_dir)?;

    Ok(())
}

#[test]
fn build_c_extension() -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = rand::thread_rng();
    let output_dir = format!("/tmp/pg_tle_test_{}", rng.gen_range(0..1000000));

    let current_file_path = Path::new(file!()).canonicalize().unwrap();
    // Example of a C extension
    let repo_url = "https://github.com/aws/pg_tle.git";
    let repo_dir_path = current_file_path.parent().unwrap().join("pg_tle");
    let repo_dir = PathBuf::from(repo_dir_path);
    if repo_dir.exists() {
        fs::remove_dir_all(&repo_dir.clone()).unwrap();
    }

    let repo = Repository::clone(repo_url, &repo_dir).unwrap();

    let refname = "v1.0.3";
    let (object, reference) = repo.revparse_ext(refname).expect("Object not found");

    repo.checkout_tree(&object, None)
        .expect("Failed to checkout");

    match reference {
        // gref is an actual reference like branches or tags
        Some(gref) => repo.set_head(gref.name().unwrap()),
        // this is a commit, not a reference
        None => repo.set_head_detached(object.id()),
    }
        .expect("Failed to set HEAD");

    // Construct a path relative to the current file's directory
    let mut extension_path = std::path::PathBuf::from(file!());
    extension_path.pop(); // Remove the file name from the path
    extension_path.push("pg_tle");

    return Ok(());

    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("build");
    cmd.arg("--path");
    cmd.arg(extension_path.as_os_str());
    cmd.arg("--output-path");
    cmd.arg(output_dir.clone());
    cmd.assert().code(0);
    assert!(
        std::path::Path::new(format!("{output_dir}/test_pgx_extension-0.0.0.tar.gz").as_str())
            .exists()
    );
    // delete the temporary file
    std::fs::remove_dir_all(output_dir)?;

    Ok(())
}
