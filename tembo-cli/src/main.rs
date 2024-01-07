use crate::cmd::delete::DeleteCommand;
use crate::cmd::validate::ValidateCommand;
use crate::cmd::{apply, context, delete, init, validate};
use clap::{crate_authors, crate_version, Args, Parser, Subcommand};
use cmd::apply::ApplyCommand;
use cmd::context::{ContextCommand, ContextSubCommand};
use cmd::init::InitCommand;

mod cli;
mod cmd;
mod tui;

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

#[derive(Args)]
struct GlobalOpts {
    /// Show more information in command output
    #[clap(short, long)]
    verbose: bool,
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
        }
        SubCommands::Apply(ref _apply_cmd) => {
            let merge_option = &_apply_cmd.merge;
            apply::execute(app.global_opts.verbose, merge_option.clone())?;

            if let SubCommands::Apply(ref _apply_cmd) = app.command {
                let overlay_path = &_apply_cmd.merge;

                let _instance_settings = apply::get_instance_settings(overlay_path.clone())?;
            }
        }
        SubCommands::Validate(_validate_cmd) => {
            validate::execute(app.global_opts.verbose)?;
        }
        SubCommands::Delete(_delete_cmd) => {
            delete::execute()?;
        }
    }

    Ok(())
}
