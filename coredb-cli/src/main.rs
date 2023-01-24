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
    Get(commands::get::GetCommand),
    Create(commands::create::CreateCommand),
    Install(commands::install::InstallCommand),
}

impl SubCommand for SubCommands {
    fn execute(&self) {
        match self {
            SubCommands::Get(cmd) => cmd.execute(),
            SubCommands::Create(cmd) => cmd.execute(),
            SubCommands::Install(cmd) => cmd.execute(),
        }
    }
}

fn main() {
    let cli = Cli::parse();
    cli.command.execute();
}
