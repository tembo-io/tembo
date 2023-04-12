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
    #[arg(long = "homepage", short = 'H')]
    homepage: Option<String>,
    #[arg(long = "license", short = 'l')]
    license: Option<String>,
    #[arg(
        long = "registry",
        short = 'r',
        default_value = "https://registry.pgtrunk.io"
    )]
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
        // Validate extension name input
        check_input(&self.name)?;
        let (file, name) = match &self.file {
            Some(..) => {
                // If file is specified, use it
                let path = self.file.clone().unwrap();
                let name = path.file_name().unwrap().to_str().unwrap().to_owned();
                let f = fs::read(self.file.clone().unwrap())?;
                (f, name)
            }
            None => {
                // If no file is specified, read file from working dir with format
                // <extension_name>-<version>.tar.gz.
                // Error if file is not found
                let mut path = PathBuf::new();
                let _ = &path.push(format!("./{}-{}.tar.gz", self.name, self.version));
                let name = path.file_name().unwrap().to_str().unwrap().to_owned();
                let f = fs::read(path.clone())?;
                (f, name)
            }
        };
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/octet-stream".parse().unwrap());
        let file_part = reqwest::multipart::Part::bytes(file)
            .file_name(name)
            .headers(headers);
        let m = json!({
            "name": self.name,
            "vers": self.version,
            "description": self.description,
            "documentation": self.documentation,
            "homepage": self.homepage,
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
        // Print response from registry
        println!("{}", res);
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
