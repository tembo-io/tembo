use crate::cli::context::{get_current_context, Target};
use crate::cmd::apply::{get_instance_id, get_instance_settings};
use anyhow::{Context, Result};
use clap::Args;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::process::Command;
use temboclient::apis::configuration::Configuration;

#[derive(Args)]
pub struct LogsCommand {}

#[derive(Serialize, Deserialize, Debug)]
struct LogStream {
    app: String,
    container: String,
    pod: String,
    stream: String,
    tembo_instance_id: String,
    tembo_organization_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct LogEntry {
    stream: LogStream,
    values: Vec<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct LogResult {
    result_type: String,
    result: Vec<LogEntry>,
}

#[derive(Serialize, Deserialize, Debug)]
struct LogData {
    status: String,
    data: LogResult,
}

#[derive(Serialize, Deserialize, Debug)]
struct IndividualLogEntry {
    ts: String,
    msg: String,
}

fn beautify_logs(json_data: &str) -> Result<()> {
    let log_data: LogData = serde_json::from_str(json_data)?;

    for entry in log_data.data.result {
        for value in entry.values {
            let log = &value[1];

            match serde_json::from_str::<IndividualLogEntry>(log) {
                Ok(log_entry) => println!("{}", format_log_entry(&log_entry)),
                Err(_) => println!("{}", log),
            }
        }
    }

    Ok(())
}

fn format_log_entry(log_entry: &IndividualLogEntry) -> String {
    format!("{} {}", log_entry.ts, log_entry.msg)
}

pub fn execute() -> Result<()> {
    let env = match get_current_context() {
        Ok(env) => env,
        Err(e) => return Err(e), // early return in case of error
    };

    if env.target == Target::Docker.to_string() {
        let instance_settings = get_instance_settings(None, None)?;
        for (_instance_name, _settings) in instance_settings {
            docker_logs(&_settings.instance_name)?;
        }
    } else if env.target == Target::TemboCloud.to_string() {
        let _ = cloud_logs();
    }
    Ok(())
}

pub fn cloud_logs() -> Result<()> {
    let env = get_current_context()?;
    let org_id = env.org_id.clone().unwrap_or_default();
    let profile = env.selected_profile.clone().unwrap();
    let tembo_data_host = profile.get_tembo_data_host();
    let config = Configuration {
        base_path: profile.get_tembo_host(),
        bearer_access_token: Some(profile.tembo_access_token.clone()),
        ..Default::default()
    };
    let instance_settings = get_instance_settings(None, None)?;
    let client = Client::new();
    let mut headers = HeaderMap::new();
    headers.insert("X-Scope-OrgID", HeaderValue::from_str(&org_id)?);
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", profile.tembo_access_token))?,
    );

    for (_key, value) in instance_settings.iter() {
        let instance_id_option = get_instance_id(&value.instance_name, &config, &env)?;

        let instance_id = if let Some(id) = instance_id_option {
            id
        } else {
            eprintln!("Instance ID not found for {}", value.instance_name);
            continue;
        };

        let query = format!("{{tembo_instance_id=\"{}\"}}", instance_id);
        let url = format!("{}/loki/api/v1/query_range", tembo_data_host);

        let response = client
            .get(url)
            .headers(headers.clone())
            .query(&[("query", &query)])
            .send()?;

        if response.status().is_success() {
            let response_body = response.text()?;
            beautify_logs(&response_body)?;
        } else {
            eprintln!("Error: {:?}", response.status());
        }
    }

    Ok(())
}

pub fn docker_logs(instance_name: &str) -> Result<()> {
    println!("\nFetching logs for instance: {}\n", instance_name);
    let output = Command::new("docker")
        .args(["logs", instance_name])
        .output()
        .with_context(|| {
            format!(
                "Failed to fetch logs for Docker container '{}'",
                instance_name
            )
        })?;

    if !output.status.success() {
        eprintln!("Error fetching logs for instance '{}'", instance_name);
        return Ok(());
    }

    let logs_stdout = String::from_utf8_lossy(&output.stdout);
    let logs_stderr = String::from_utf8_lossy(&output.stderr);

    println!("{}{}", logs_stdout, logs_stderr);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use assert_cmd::prelude::*;
    use std::env;
    use std::error::Error;
    use std::path::PathBuf;

    const ROOT_DIR: &str = env!("CARGO_MANIFEST_DIR");
    const CARGO_BIN: &str = "tembo";

    #[tokio::test]
    async fn docker_logs() -> Result<(), Box<dyn Error>> {
        let test_dir = PathBuf::from(ROOT_DIR).join("examples").join("set");

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

        // tembo logs
        let mut cmd = Command::cargo_bin(CARGO_BIN)?;
        cmd.arg("logs");
        cmd.assert().success();

        // tembo delete
        let mut cmd = Command::cargo_bin(CARGO_BIN)?;
        cmd.arg("delete");
        let _ = cmd.ok();

        Ok(())
    }

    fn mock_query(query: &str) -> Result<String> {
        match query {
            "valid_json" => Ok(r#"{
                "status": "success",
                "data": {
                    "resultType": "matrix",
                    "result": [
                        {
                            "stream": {
                                "app": "test_app",
                                "container": "test_container",
                                "pod": "test_pod",
                                "stream": "test_stream",
                                "tembo_instance_id": "test_id",
                                "tembo_organization_id": "test_org_id"
                            },
                            "values": [
                                ["1234567890", "{\"ts\":\"2024-01-24T21:37:53Z\",\"msg\":\"Valid JSON log entry\"}"]
                            ]
                        },
                        {
                            "stream": {
                                "app": "test_app",
                                "container": "test_container",
                                "pod": "test_pod",
                                "stream": "test_stream",
                                "tembo_instance_id": "test_id",
                                "tembo_organization_id": "test_org_id"
                            },
                            "values": [
                                ["1234567890", "Non-JSON log entry"]
                            ]
                        }
                    ]
                }
            }"#.to_string()),
            _ => Err(anyhow!("Invalid query")),
        }
    }

    #[tokio::test]
    async fn cloud_logs() {
        let valid_json_log = mock_query("valid_json").unwrap();
        beautify_logs(&valid_json_log).unwrap();
    }
}
