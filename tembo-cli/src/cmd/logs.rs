use crate::cli::context::{get_current_context, Target};
use crate::cmd::apply::{get_instance_id, get_instance_settings};
use anyhow::anyhow;
use anyhow::{Context, Result};
use chrono::DateTime;
use chrono::{LocalResult, TimeZone, Utc};
use clap::Args;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::process::Command;
use temboclient::apis::configuration::Configuration;

/// View logs for your instance
#[derive(Args)]
pub struct LogsCommand {
    /// Fetch logs for specific apps
    #[clap(long)]
    app: Option<String>,
}

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
struct Value2 {
    level: String,
    ts: String,
    logger: String,
    msg: String,
    logging_pod: String,
    record: Record,
}

#[derive(Serialize, Deserialize, Debug)]
struct Record {
    log_time: String,
    message: String,
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
    info: String,
}

pub fn execute(args: LogsCommand) -> Result<(), anyhow::Error> {
    let env = match get_current_context() {
        Ok(env) => env,
        Err(e) => return Err(anyhow!(e)),
    };

    if env.target == Target::Docker.to_string() {
        let instance_settings = get_instance_settings(None, None)?;
        for (_instance_name, _settings) in instance_settings {
            docker_logs(&_settings.instance_name)?;
        }
    } else if env.target == Target::TemboCloud.to_string() {
        let _ = cloud_logs(args.app)?;
    }
    Ok(())
}

fn beautify_logs(json_data: &str, app_name: Option<String>) -> Result<()> {
    let log_data: LogData = serde_json::from_str(json_data)?;
    let mut entries: BTreeMap<DateTime<Utc>, Vec<String>> = BTreeMap::new();

    for entry in &log_data.data.result {
        if app_name
            .as_ref()
            .map_or(true, |app| entry.stream.container == *app)
        {
            for value in &entry.values {
                match value[0].parse::<i64>() {
                    Ok(unix_timestamp_ns) => {
                        let unix_timestamp = unix_timestamp_ns / 1_000_000_000;
                        match Utc.timestamp_opt(unix_timestamp, 0) {
                            LocalResult::Single(date_time) => {
                                let log_detail = match serde_json::from_str::<Value2>(&value[1]) {
                                    Ok(log_details) => format!(
                                        "{} {}: ({}) {}",
                                        date_time.format("%Y-%m-%d %H:%M:%S"),
                                        log_details.level,
                                        log_details.msg,
                                        log_details.record.message
                                    ),
                                    Err(_) => format!(
                                        "{} {}",
                                        date_time.format("%Y-%m-%d %H:%M:%S"),
                                        &value[1]
                                    ),
                                };
                                entries
                                    .entry(date_time)
                                    .or_insert_with(Vec::new)
                                    .push(log_detail);
                            }
                            _ => eprintln!("Invalid or ambiguous timestamp: {}", unix_timestamp),
                        }
                    }
                    Err(e) => eprintln!("Error parsing string to i64: {}", e),
                }
            }
        }
    }

    for (_date_time, logs) in &entries {
        for log in logs {
            println!("{}", log);
        }
    }

    if app_name.is_some() && entries.is_empty() {
        return Err(anyhow!("Couldn't find logs with the specified app"));
    }

    Ok(())
}

pub fn cloud_logs(app: Option<String>) -> Result<(), anyhow::Error> {
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
            beautify_logs(&response_body, app.clone())?;
        } else {
            eprintln!("Error: {:?}", response.status());
        }
    }

    Ok(())
}

fn format_log_line(line: &str) -> Option<String> {
    if line.trim().is_empty() {
        None
    } else if line.contains("LOG:") {
        Some(line.to_string())
    } else {
        Some(format!("System Message: {}", line))
    }
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

    let all_logs = format!("{}{}", logs_stdout, logs_stderr);

    all_logs
        .lines()
        .filter_map(format_log_line)
        .for_each(|line| println!("{}", line));

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

        env::set_current_dir(test_dir)?;

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
        beautify_logs(&valid_json_log, None).unwrap();
    }
}
