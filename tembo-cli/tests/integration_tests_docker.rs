use assert_cmd::prelude::*; // Add methods on commands

use colorful::core::StrMarker;
use core::result::Result::Ok;
use curl::easy::Easy;
use predicates::prelude::*;
use sqlx::postgres::PgConnectOptions;
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use tembo::cli::sqlx_utils::SqlxUtils;
use test_case::test_case;

const CARGO_BIN: &str = "tembo";

#[test]
fn help() -> Result<(), anyhow::Error> {
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;

    cmd.arg("--help");
    cmd.assert().stdout(predicate::str::contains("Usage: "));

    Ok(())
}

#[test_case(14, "Standard")]
#[test_case(15, "Standard")]
#[test_case(16, "Standard")]
#[test_case(14, "Geospatial")]
#[test_case(15, "Geospatial")]
#[test_case(16, "Geospatial")]
#[test_case(14, "MachineLearning")]
#[test_case(15, "MachineLearning")]
#[test_case(16, "MachineLearning")]
#[test_case(14, "MessageQueue")]
#[test_case(15, "MessageQueue")]
#[test_case(16, "MessageQueue")]
#[test_case(14, "MongoAlternative")]
#[test_case(15, "MongoAlternative")]
#[test_case(16, "MongoAlternative")]
#[test_case(14, "OLTP")]
#[test_case(15, "OLTP")]
#[test_case(16, "OLTP")]
#[test_case(14, "VectorDB")]
#[test_case(15, "VectorDB")]
#[test_case(16, "VectorDB")]
#[tokio::test]
#[ignore]
async fn minimal(version: i32, stack_type: &str) -> Result<(), anyhow::Error> {
    if let Err(_err) = verify_minimal(version, stack_type).await {
        teardown_minimal(version, stack_type)?;

        assert!(false);
    }

    teardown_minimal(version, stack_type)?;

    Ok(())
}

fn teardown_minimal(version: i32, stack_type: &str) -> Result<(), anyhow::Error> {
    // tembo delete
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    let _ = cmd.ok();

    replace_vars_in_file(
        "tembo.toml".to_string(),
        &format!("pg_version = {version}"),
        "pg_version = 15",
    )?;

    replace_vars_in_file(
        "tembo.toml".to_string(),
        &format!("stack_type = \"{stack_type}\""),
        "stack_type = \"Standard\"",
    )?;
    Ok(())
}

async fn verify_minimal(version: i32, stack_type: &str) -> Result<(), anyhow::Error> {
    let root_dir = env!("CARGO_MANIFEST_DIR");
    let test_dir = PathBuf::from(root_dir).join("examples").join("minimal");

    env::set_current_dir(&test_dir)?;

    replace_vars_in_file(
        "tembo.toml".to_string(),
        "pg_version = 15",
        &format!("pg_version = {version}"),
    )?;

    replace_vars_in_file(
        "tembo.toml".to_string(),
        "stack_type = \"Standard\"",
        &format!("stack_type = \"{stack_type}\""),
    )?;

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
    let output = cmd.assert().try_success();

    match output {
        Ok(output) => output,
        Err(err) => {
            return Err(err.into());
        }
    };

    // check can connect
    assert_can_connect("minimal".to_str()).await?;

    Ok(())
}

#[tokio::test]
async fn vector() -> Result<(), anyhow::Error> {
    let root_dir = env!("CARGO_MANIFEST_DIR");
    let test_dir = PathBuf::from(root_dir).join("examples").join("vector");

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
    assert_can_connect("vector".to_str()).await?;

    // tembo delete
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    let _ = cmd.ok();

    let mut cmd = Command::new("sh");
    cmd.arg("-c");
    cmd.arg("docker volume rm $(docker volume ls -q)");
    cmd.assert().success();

    // check can't connect
    assert!(assert_can_connect("vector".to_str()).await.is_err());

    Ok(())
}

#[tokio::test]
async fn migrations() -> Result<(), anyhow::Error> {
    let root_dir = env!("CARGO_MANIFEST_DIR");
    let test_dir = PathBuf::from(root_dir)
        .join("examples")
        .join("migrations-1");

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

    // tembo delete
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    let _ = cmd.ok();

    let mut cmd = Command::new("sh");
    cmd.arg("-c");
    cmd.arg("docker volume rm $(docker volume ls -q)");
    cmd.assert().success();

    // check can't connect
    assert!(assert_can_connect("migration-test".to_str()).await.is_err());

    Ok(())
}

#[tokio::test]
async fn analytics() -> Result<(), anyhow::Error> {
    let instance_name = "analytics";

    let root_dir = env!("CARGO_MANIFEST_DIR");
    let test_dir = PathBuf::from(root_dir).join("examples").join(instance_name);

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
    let result: String = get_output_from_sql(
        instance_name.to_string(),
        "SELECT 1 FROM pg_extension WHERE extname = 'clerk_fdw'".to_string(),
    )
    .await?;
    assert!(result.contains('1'), "Query did not return 1");

    // tembo delete
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    let _ = cmd.ok();

    let mut cmd = Command::new("sh");
    cmd.arg("-c");
    cmd.arg("docker volume rm $(docker volume ls -q)");
    cmd.assert().success();

    // check can't connect
    assert!(assert_can_connect(instance_name.to_string()).await.is_err());

    Ok(())
}

#[tokio::test]
async fn multiple_instances() -> Result<(), anyhow::Error> {
    let instance1_name = "instance-1";
    let instance2_name = "instance-2";

    let root_dir = env!("CARGO_MANIFEST_DIR");
    let test_dir = PathBuf::from(root_dir)
        .join("examples")
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

    sleep(Duration::from_secs(5));

    // check can connect
    assert_can_connect(instance1_name.to_string()).await?;
    assert_can_connect(instance2_name.to_string()).await?;

    SqlxUtils::execute_sql(
        instance2_name.to_string(),
        "create table public.todos (id serial primary key,
            done boolean not null default false,
            task text not null,
            due timestamptz
          );"
        .to_string(),
    )
    .await?;

    SqlxUtils::execute_sql(
        instance2_name.to_string(),
        "insert into public.todos (task) values
        ('finish tutorial 0'), ('pat self on back');"
            .to_string(),
    )
    .await?;

    let mut easy = Easy::new();
    easy.url(&format!(
        "http://{}.local.tembo.io:8000/restapi/v1/todos",
        instance2_name
    ))
    .unwrap();
    easy.perform().unwrap();
    assert_eq!(easy.response_code().unwrap(), 200);

    // tembo delete
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    let _ = cmd.ok();

    let mut cmd = Command::new("sh");
    cmd.arg("-c");
    cmd.arg("docker volume rm $(docker volume ls -q)");
    cmd.assert().success();

    // check can't connect
    assert!(assert_can_connect(instance1_name.to_str()).await.is_err());
    assert!(assert_can_connect(instance2_name.to_str()).await.is_err());

    Ok(())
}

#[tokio::test]
async fn local_persistence() -> Result<(), anyhow::Error> {
    let instance_name = "set";
    let root_dir = env!("CARGO_MANIFEST_DIR");
    let test_dir = PathBuf::from(root_dir).join("examples").join(instance_name);

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

    // Create a table and insert data
    SqlxUtils::execute_sql(
        instance_name.to_string(),
        "CREATE TABLE test_table (id serial PRIMARY KEY, data TEXT NOT NULL);".to_string(),
    )
    .await?;

    SqlxUtils::execute_sql(
        instance_name.to_string(),
        "INSERT INTO test_table (data) VALUES ('test data');".to_string(),
    )
    .await?;

    // Stop the container
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    let _ = cmd.ok();

    // Start the container again
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("--verbose");
    cmd.arg("apply");
    cmd.assert().success();

    SqlxUtils::execute_sql(
        instance_name.to_string(),
        "SELECT * FROM test_table;".to_string(),
    )
    .await?;

    // Stop the container
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    let _ = cmd.ok();

    let mut cmd = Command::new("sh");
    cmd.arg("-c");
    cmd.arg("docker volume rm $(docker volume ls -q)");
    cmd.assert().success();

    Ok(())
}

#[tokio::test]
async fn run_migration_secret() -> Result<(), anyhow::Error> {
    let instance_name = "migrations-2";
    let root_dir = env!("CARGO_MANIFEST_DIR");
    let test_dir = PathBuf::from(root_dir).join("examples").join(instance_name);

    env::set_current_dir(&test_dir)?;

    // Set the environment variable
    env::set_var("TEMBO_CUSTOM_SECRET", "my_custom_secret_value");

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

    // Stop the container
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    let _ = cmd.ok();

    Ok(())
}

async fn get_output_from_sql(instance_name: String, sql: String) -> Result<String, anyhow::Error> {
    // Configure SQLx connection options
    let connect_options = PgConnectOptions::new()
        .username("postgres")
        .password("postgres")
        .host(&format!("{}.local.tembo.io", instance_name))
        .database("postgres");

    // Connect to the database
    let pool = sqlx::PgPool::connect_with(connect_options).await?;

    // Simple query
    let result: (i32,) = sqlx::query_as(&sql).fetch_one(&pool).await?;

    println!(
        "Successfully connected to the database: {}",
        &format!("{}.local.tembo.io", instance_name)
    );
    println!("{}", result.0);

    Ok(result.0.to_string())
}

async fn assert_can_connect(instance_name: String) -> Result<(), anyhow::Error> {
    let result: String = get_output_from_sql(instance_name, "SELECT 1".to_string()).await?;
    assert!(result.contains('1'), "Query did not return 1");
    Ok(())
}

pub fn replace_vars_in_file(
    file_path: String,
    word_from: &str,
    word_to: &str,
) -> Result<(), anyhow::Error> {
    let mut src = File::open(&file_path)?;
    let mut data = String::new();
    src.read_to_string(&mut data)?;
    drop(src);
    let new_data = data.replace(word_from, word_to);
    let mut dst = File::create(&file_path)?;
    dst.write_all(new_data.as_bytes())?;
    Ok(())
}
