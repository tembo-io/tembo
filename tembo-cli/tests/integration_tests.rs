use assert_cmd::prelude::*; // Add methods on commands

use colorful::core::StrMarker;
use curl::easy::Easy;
use predicates::prelude::*;
use sqlx::postgres::PgConnectOptions;
use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

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
    let test_dir = PathBuf::from(root_dir).join("examples").join("minimal");

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
    let _ = cmd.ok();

    // check can't connect
    assert!(assert_can_connect("minimal".to_str()).await.is_err());

    Ok(())
}

#[tokio::test]
async fn data_warehouse() -> Result<(), Box<dyn Error>> {
    let instance_name = "data-warehouse";

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

    // check can't connect
    assert!(assert_can_connect(instance_name.to_string()).await.is_err());

    Ok(())
}

#[tokio::test]
async fn multiple_instances() -> Result<(), Box<dyn Error>> {
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

    execute_sql(
        instance2_name.to_string(),
        "create table public.todos (id serial primary key,
            done boolean not null default false,
            task text not null,
            due timestamptz
          );"
        .to_string(),
    )
    .await?;

    execute_sql(
        instance2_name.to_string(),
        "insert into public.todos (task) values
        ('finish tutorial 0'), ('pat self on back');"
            .to_string(),
    )
    .await?;

    let mut easy = Easy::new();
    easy.url(&format!(
        "http://{}.local.tembo.io:8000/restapi/v1/todos",
        instance2_name.to_string()
    ))
    .unwrap();
    easy.perform().unwrap();
    assert_eq!(easy.response_code().unwrap(), 200);

    // tembo delete
    let mut cmd = Command::cargo_bin(CARGO_BIN)?;
    cmd.arg("delete");
    let _ = cmd.ok();

    // check can't connect
    assert!(assert_can_connect(instance1_name.to_str()).await.is_err());
    assert!(assert_can_connect(instance2_name.to_str()).await.is_err());

    Ok(())
}

async fn get_output_from_sql(instance_name: String, sql: String) -> Result<String, Box<dyn Error>> {
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

async fn execute_sql(instance_name: String, sql: String) -> Result<(), Box<dyn Error>> {
    // Configure SQLx connection options
    let connect_options = PgConnectOptions::new()
        .username("postgres")
        .password("postgres")
        .host(&format!("{}.local.tembo.io", instance_name))
        .database("postgres");

    // Connect to the database
    let pool = sqlx::PgPool::connect_with(connect_options).await?;

    // Simple query
    sqlx::query(&sql).fetch_optional(&pool).await?;

    println!(
        "Successfully connected to the database: {}",
        &format!("{}.local.tembo.io", instance_name)
    );

    Ok(())
}

async fn assert_can_connect(instance_name: String) -> Result<(), Box<dyn Error>> {
    let result: String = get_output_from_sql(instance_name, "SELECT 1".to_string()).await?;
    assert!(result.contains('1'), "Query did not return 1");
    Ok(())
}
