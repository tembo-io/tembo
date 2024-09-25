use crate::cli::context::{get_current_context, Target};
use crate::cmd::apply::{get_instance_id, get_instance_settings};
use anyhow::anyhow;
use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use chrono::DateTime;
use chrono::{LocalResult, TimeZone, Utc};
use clap::Args;
use futures_util::StreamExt;
use rand::Rng;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::process::{Command, Output};
use temboclient::apis::configuration::Configuration;
use tokio_tungstenite::tungstenite::protocol::Message;
use tungstenite::http::header::{
    HeaderName as TungsteniteHeaderName, HeaderValue as TungsteniteHeaderValue,
};
use tungstenite::http::Request;
use url::Url;

/// View logs for your instance
#[derive(Args)]
pub struct LogsCommand {
    /// Tail your logs
    #[clap(long, action = clap::ArgAction::SetTrue)]
    tail: bool,

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

#[tokio::main]
pub async fn execute(args: LogsCommand) -> Result<(), anyhow::Error> {
    let env = match get_current_context() {
        Ok(env) => env,
        Err(e) => return Err(anyhow!(e)),
    };

    if env.target == Target::Docker.to_string() {
        let instance_settings = get_instance_settings(None, None)?;
        for (_instance_name, _settings) in instance_settings {
            docker_logs(&_settings.instance_name, args.tail)?;
        }
    } else if env.target == Target::TemboCloud.to_string() {
        cloud_logs(args.tail, args.app).await?;
    }
    Ok(())
}

async fn cloud_logs(tail: bool, app: Option<String>) -> Result<(), anyhow::Error> {
    let env_result = get_current_context()?;
    let org_id = env_result.org_id.clone().unwrap_or_default();
    let profile = env_result.selected_profile.clone().unwrap();
    let tembo_data_host = profile.get_tembo_data_host();
    let tembo_access_token = profile.tembo_access_token.clone();

    let config = Configuration {
        base_path: profile.get_tembo_host(),
        bearer_access_token: Some(tembo_access_token.clone()),
        ..Default::default()
    };

    let instance_settings_result = get_instance_settings(None, None)?;

    for (_key, value) in instance_settings_result.iter() {
        let headers = build_headers(&org_id, &tembo_access_token)?;
        let result_clone = env_result.clone();
        let instance_name = value.instance_name.clone();
        let config_clone = config.clone();

        let instance_id_option = tokio::task::spawn_blocking(move || {
            get_instance_id(&instance_name, &config_clone, &result_clone)
        })
        .await
        .context("Failed to get instance ID")?
        .context("Failed to get instance ID")?;

        if let Some(instance_id) = instance_id_option {
            if tail {
                fetch_logs_websocket(&headers, instance_id).await?;
            } else {
                fetch_logs_query(&tembo_data_host, &headers, instance_id, app.clone()).await?;
            }
        } else {
            eprintln!("Instance ID not found for {}", value.instance_name);
        }
    }

    Ok(())
}

fn build_headers(org_id: &str, tembo_access_token: &str) -> Result<HeaderMap, anyhow::Error> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "X-Scope-OrgID",
        reqwest::header::HeaderValue::from_str(org_id)?,
    );
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("Bearer {}", tembo_access_token))?,
    );
    Ok(headers)
}

async fn fetch_logs_websocket(
    headers: &reqwest::header::HeaderMap,
    instance_id: String,
) -> Result<(), anyhow::Error> {
    let query = format!("{{tembo_instance_id=\"{}\"}}", instance_id);
    let url_encoded_query = urlencoding::encode(&query);
    let ws_url = format!(
        "wss://api.data-1.use1.tembo.io/loki/api/v1/tail?query={}",
        url_encoded_query
    );
    let mut key = [0u8; 16];
    rand::thread_rng().fill(&mut key);
    let sec_websocket_key = general_purpose::STANDARD.encode(key);

    let url = Url::parse(&ws_url)?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("Invalid URL: missing host"))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| anyhow!("Invalid URL: missing port"))?;

    let mut request_builder = Request::builder()
        .uri(url.to_string())
        .method("GET")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", sec_websocket_key)
        .header("Host", format!("{}:{}", host, port));

    for (name, value) in headers.iter() {
        let header_name = TungsteniteHeaderName::from_bytes(name.as_str().as_bytes())
            .map_err(|_| anyhow!("Invalid header name: {}", name))?;
        let header_value = TungsteniteHeaderValue::from_bytes(value.as_bytes())
            .map_err(|_| anyhow!("Invalid header value for {}", name.as_str()))?;
        request_builder = request_builder.header(header_name, header_value);
    }

    let request = request_builder
        .body(())
        .map_err(|e| anyhow!("Failed to build request: {}", e))?;

    let (mut ws_stream, _) = tokio_tungstenite::connect_async(request).await?;

    while let Some(message) = ws_stream.next().await {
        match message? {
            Message::Text(text) => {
                beautify_logs(&text, None)?;
            }
            Message::Close(_) => {
                println!("WebSocket connection closed by server");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

async fn fetch_logs_query(
    tembo_data_host: &str,
    headers: &HeaderMap,
    instance_id: String,
    app_name: Option<String>,
) -> Result<(), anyhow::Error> {
    let client = reqwest::Client::new();
    let query = format!("{{tembo_instance_id=\"{}\"}}", instance_id);
    let url = format!("{}/loki/api/v1/query_range", tembo_data_host);

    let response = client
        .get(url)
        .headers(headers.clone())
        .query(&[("query", &query)])
        .send()
        .await
        .context("Failed to send query request")?;

    if response.status().is_success() {
        let response_body = response
            .text()
            .await
            .context("Failed to read response body")?;
        beautify_logs(&response_body, app_name)?;
    } else {
        eprintln!("Error: {:?}", response.status());
    }

    Ok(())
}

pub fn docker_logs(instance_name: &str, tail: bool) -> Result<()> {
    if tail {
        println!("\nFetching logs for instance: {}\n", instance_name);
        let output = Command::new("docker")
            .args(["logs", "--follow", instance_name])
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

        print_docker_logs(output)?;
    } else {
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
        print_docker_logs(output)?;
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
                                entries.entry(date_time).or_default().push(log_detail);
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

fn format_log_line(line: &str) -> Option<String> {
    if line.trim().is_empty() {
        None
    } else if line.contains("LOG:") {
        Some(line.to_string())
    } else {
        Some(format!("System Message: {}", line))
    }
}

fn print_docker_logs(output: Output) -> Result<(), anyhow::Error> {
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

        let mut cmd = Command::new("sh");
        cmd.arg("-c");
        cmd.arg("docker volume rm $(docker volume ls -q)");
        cmd.assert().success();

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
