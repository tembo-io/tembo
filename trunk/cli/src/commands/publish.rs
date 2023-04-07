use super::SubCommand;
use crate::commands::publish::PublishError::InvalidExtensionName;
use async_trait::async_trait;
use clap::Args;
use hyper::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tokio_task_manager::Task;

#[derive(Args)]
pub struct PublishCommand {
    name: String,
    #[arg(long = "version", short = 'v')]
    version: String,
    #[arg(long = "file", short = 'f')]
    file: Option<PathBuf>,
    #[arg(long = "description", short = 'd')]
    description: Option<String>,
    #[arg(long = "documentation", short = 'D')]
    documentation: Option<String>,
    #[arg(long = "license", short = 'l')]
    license: Option<String>,
    #[arg(long = "registry", short = 'r', default_value = "https://pgtrunk.io")]
    registry: String,
    #[arg(long = "repository", short = 'R')]
    repository: Option<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum PublishError {
    #[error("extension name can include alphanumeric characters and underscores")]
    InvalidExtensionName,
}

#[async_trait]
impl SubCommand for PublishCommand {
    async fn execute(&self, _task: Task) -> Result<(), anyhow::Error> {
        if let Some(ref file) = self.file {
            check_input(&self.name)?;
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, "application/octet-stream".parse().unwrap());
            let file = fs::read(file).unwrap();
            let file_part = reqwest::multipart::Part::bytes(file)
                .file_name("pgmq-0.2.1.tar.gz")
                .headers(headers);
            let m = json!({
                "name": self.name,
                "vers": self.version,
                "description": self.description,
                "documentation": self.documentation,
                "license": self.license,
                "repository": self.repository
            });
            let metadata = reqwest::multipart::Part::text(m.to_string());
            let form = reqwest::multipart::Form::new()
                .part("metadata", metadata)
                .part("file", file_part);
            let client = reqwest::Client::new();
            let url = format!("{}/extensions/new", self.registry);
            let res = client
                .post(url)
                .multipart(form)
                .send()
                .await?
                .text()
                .await?;
            // print response from registry
            println!("{}", res)
        }
        Ok(())
    }
}

pub fn check_input(input: &str) -> Result<(), PublishError> {
    let valid = input
        .as_bytes()
        .iter()
        .all(|&c| c.is_ascii_alphanumeric() || c == b'_');
    match valid {
        true => Ok(()),
        false => Err(InvalidExtensionName),
    }
}
