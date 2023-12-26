use clap::{ArgMatches, Args, crate_authors, crate_version, Parser, Subcommand};
use cmd::apply::ApplyCommand;
use cmd::context::{ContextCommand, ContextSubCommand};
use cmd::init::InitCommand;
use crate::cmd::{apply, context, delete, init, validate};
use crate::cmd::delete::DeleteCommand;
use crate::cmd::validate::ValidateCommand;

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
    Validate(ValidateCommand),
    Delete(DeleteCommand),
}

// Global options available to all subcommands
#[derive(Args)]
struct GlobalOpts {
    // Define global options here
    // Example: Verbose mode
    #[clap(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn main() -> Result<(), anyhow::Error> {
    let app = App::parse();

    match app.command {
        SubCommands::Context(context_cmd) => match context_cmd.subcommand {
            ContextSubCommand::List => {
                context::list::execute()?;
            }
            ContextSubCommand::Set(args) => {
                context::set::execute(&args)?;
            }
        },
        SubCommands::Init(_init_cmd) => {
            init::execute()?;
        },
        SubCommands::Apply(_apply_cmd) => {
            apply::execute()?;
        },
        SubCommands::Validate(_validate_cmd) => {
            validate::execute()?;
        },
        SubCommands::Delete(_delete_cmd) => {
            delete::execute()?;
        },
    }

    return Ok(());
}
