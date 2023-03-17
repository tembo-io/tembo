mod commands;
mod manifest;
mod sync_utils;

use crate::commands::SubCommand;
use async_trait::async_trait;
use clap::{Parser, Subcommand};
use std::time::Duration;
use tokio_task_manager::{Task, TaskManager};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = false)]
struct Cli {
    #[command(subcommand)]
    command: SubCommands,
}

#[derive(Subcommand)]
enum SubCommands {
    Build(commands::build::BuildCommand),
    Publish(commands::publish::PublishCommand),
    Install(commands::install::InstallCommand),
}

#[async_trait]
impl SubCommand for SubCommands {
    async fn execute(&self, task: Task) -> Result<(), anyhow::Error> {
        match self {
            SubCommands::Build(cmd) => cmd.execute(task).await,
            SubCommands::Publish(cmd) => cmd.execute(task).await,
            SubCommands::Install(cmd) => cmd.execute(task).await,
        }
    }
}

fn main() {
    let tm = TaskManager::new(Duration::from_secs(60));

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let cli = Cli::parse();
        let result = cli.command.execute(tm.task()).await;
        tm.wait().await;
        result
    })
    .expect("error occurred");
}
