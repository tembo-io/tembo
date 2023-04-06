use std::fs;
use std::path::PathBuf;
use super::SubCommand;
use async_trait::async_trait;
use clap::Args;
use hyper::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
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

#[async_trait]
impl SubCommand for PublishCommand {
    async fn execute(&self, _task: Task) -> Result<(), anyhow::Error> {
        if let Some(ref file) = self.file {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, "application/octet-stream".parse().unwrap());
            let file = fs::read(file).unwrap();
            let file_part = reqwest::multipart::Part::bytes(file).file_name("pgmq-0.2.1.tar.gz").headers(headers);
            let metadata = reqwest::multipart::Part::text("{\"name\": \"pgmq\", \"vers\": \"0.2.1\", \"description\": \"A lightweight distributed message queue. Like AWS SQS and RSMQ but on Postgres.\", \"documentation\": null, \"license\": \"Apache-2.0\", \"repository\": \"https://github.com/CoreDB-io/coredb/tree/main/extensions/pgmq\"}");
            let form = reqwest::multipart::Form::new().part("metadata", metadata).part("file", file_part);
            let client = reqwest::Client::new();
            let url = format!("{}/extensions/new", self.registry);
            let res = client.post(url).multipart(form).send().await?;
            println!("RES: {:?}", res);
        }
        Ok(())
    }
}
