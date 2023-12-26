use clap::{Args, crate_authors, crate_version, Parser, Subcommand};
use cmd::apply::ApplyCommand;
use cmd::context::{ContextCommand, ContextSubCommand};
use cmd::init::InitCommand;

mod cmd;
mod cli;

#[derive(Parser)]
#[clap(author = crate_authors!("\n"), version = crate_version!(), about = "Tembo CLI", long_about = None)]
struct App {
    #[clap(flatten)]
    global_opts: GlobalOpts,

    #[clap(subcommand)]
    command: SubCommands,
}

// Enum representing all available commands
#[derive(Subcommand)]
enum SubCommands {
    Context(ContextCommand),
    Init(InitCommand),
    Apply(ApplyCommand),
}

// Global options available to all subcommands
#[derive(Args)]
struct GlobalOpts {
    // Define global options here
    // Example: Verbose mode
    #[clap(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn main() {
    let app = App::parse();

    match app.command {
        SubCommands::Context(context_cmd) => match context_cmd.subcommand {
            ContextSubCommand::List => {
                // Implement logic for 'context list' here
                println!("Listing context...");
            }
            ContextSubCommand::Set(args) => {
                // Implement logic for 'context set' here
                println!("Setting context to: {}", args.name);
            }
        },
        SubCommands::Init(_) => {
            // Implement logic for 'init' command
            println!("Initializing...");
        },
        SubCommands::Apply(_) => {
            // Implement logic for 'apply' command
            println!("Applying...");
        },
        // ... other top-level commands
    }
}
