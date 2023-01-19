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
}

impl SubCommand for SubCommands {
    fn execute(&self) {
        match self {
            SubCommands::Get(cmd) => cmd.execute(),
            SubCommands::Create(cmd) => cmd.execute(),
        }
    }
}

fn main() {
    let cli = Cli::parse();
    cli.command.execute();
}
