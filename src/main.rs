mod commands;

use crate::commands::SubCommand;
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

impl SubCommand for SubCommands {
    fn execute(&self) {
        match self {
            SubCommands::Build(cmd) => cmd.execute(),
            SubCommands::Publish(cmd) => cmd.execute(),
            SubCommands::Install(cmd) => cmd.execute(),
        }
    }
}

fn main() {
    let cli = Cli::parse();
    cli.command.execute();
}
