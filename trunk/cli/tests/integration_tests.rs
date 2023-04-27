use assert_cmd::prelude::*; // Add methods on commands
use git2::Repository;
use predicates::prelude::*; // Used for writing assertions
use rand::Rng;
use std::fs;
use std::path::Path;
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
fn install_manifest_v1_extension() -> Result<(), Box<dyn std::error::Error>> {
    // Construct a path relative to the current file's directory
    let mut extension_path = std::path::PathBuf::from(file!());
    extension_path.pop(); // Remove the file name from the path
    extension_path.push("artifact-v1/my_extension-0.0.0.tar.gz");

    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("install");
    cmd.arg("--file");
    cmd.arg(extension_path.as_os_str());
    cmd.arg("--version");
    cmd.arg("0.0.0");
    cmd.arg("my_extension");
    cmd.assert().code(0);

    // Get output of 'pg_config --sharedir'
    let output = Command::new("pg_config")
        .arg("--sharedir")
        .output()
        .expect("failed to find sharedir, is pg_config in path?");
    let sharedir = String::from_utf8(output.stdout)?;
    let sharedir = sharedir.trim();

    let output = Command::new("pg_config")
        .arg("--pkglibdir")
        .output()
        .expect("failed to find pkglibdir, is pg_config in path?");
    let pkglibdir = String::from_utf8(output.stdout)?;
    let pkglibdir = pkglibdir.trim();

    assert!(
        std::path::Path::new(format!("{sharedir}/extension/my_extension.control").as_str())
            .exists()
    );
    assert!(
        std::path::Path::new(format!("{sharedir}/extension/my_extension--0.0.0.sql").as_str())
            .exists()
    );
    assert!(std::path::Path::new(format!("{pkglibdir}/my_extension.so").as_str()).exists());
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
    // clone and checkout ref v1.0.3
    let repo_dir_path = current_file_path.parent().unwrap().join("pg_tle");
    let repo_dir = repo_dir_path;
    if repo_dir.exists() {
        fs::remove_dir_all(repo_dir.clone()).unwrap();
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

    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("build");
    cmd.arg("--path");
    cmd.arg(extension_path.as_os_str());
    cmd.arg("--output-path");
    cmd.arg(output_dir.clone());
    cmd.arg("--version");
    cmd.arg("1.0.3");
    cmd.arg("--name");
    cmd.arg("pg_tle");
    cmd.assert().code(0);
    assert!(std::path::Path::new(format!("{output_dir}/pg_tle-1.0.3.tar.gz").as_str()).exists());
    // delete the temporary file
    std::fs::remove_dir_all(output_dir)?;

    Ok(())
}
