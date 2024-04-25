use crate::cmd::delete::DeleteCommand;
use crate::cmd::validate::ValidateCommand;
use crate::cmd::{apply, context, delete, init, login, logs, top, validate};
use clap::{crate_authors, crate_version, Args, Parser, Subcommand};
use cmd::apply::ApplyCommand;
use cmd::context::{ContextCommand, ContextSubCommand};
use cmd::init::InitCommand;
use cmd::login::LoginCommand;
use cmd::logs::LogsCommand;
use cmd::top::TopCommand;

mod cli;
mod cmd;
mod tui;

#[derive(Parser)]
#[clap(name = "tembo", author = crate_authors!("\n"), version = crate_version!(), long_about = None)]
struct App {
    #[arg(long, hide = true)]
    markdown_help: bool,

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
    Logs(LogsCommand),
    Login(LoginCommand),
    Top(TopCommand),
}

#[derive(Args)]
struct GlobalOpts {
    /// Show more information in command output
    #[clap(short, long)]
    verbose: bool,
}

fn main() -> Result<(), anyhow::Error> {

    if std::env::args().any(|arg| arg == "--markdown-help") {
        clap_markdown::print_help_markdown::<App>();
        return Ok(());
    }
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
        SubCommands::Apply(_apply_cmd) => {
            apply::execute(
                app.global_opts.verbose,
                _apply_cmd.merge.clone(),
                _apply_cmd.set.clone(),
            )?;
        }
        SubCommands::Validate(_validate_cmd) => {
            validate::execute(app.global_opts.verbose)?;
        }
        SubCommands::Logs(_logs_cmd) => {
            logs::execute()?;
        }
        SubCommands::Delete(_delete_cmd) => {
            delete::execute()?;
        }
        SubCommands::Login(_login_cmd) => {
            login::execute(_login_cmd)?;
        }
        SubCommands::Top(_top_cmd) => {
            top::execute(app.global_opts.verbose, _top_cmd)?;
        }
    }

    Ok(())
}
