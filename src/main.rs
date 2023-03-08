mod commands;
mod manifest;
mod sync_utils;

use crate::commands::SubCommand;
use async_trait::async_trait;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
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
    async fn execute(&self) -> Result<(), anyhow::Error> {
        match self {
            SubCommands::Build(cmd) => cmd.execute().await,
            SubCommands::Publish(cmd) => cmd.execute().await,
            SubCommands::Install(cmd) => cmd.execute().await,
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    cli.command.execute().await.unwrap();
}
