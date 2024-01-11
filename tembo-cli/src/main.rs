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
        SubCommands::Apply(_apply_cmd) => {
            apply::execute(app.global_opts.verbose, _apply_cmd.merge.clone())?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    const CARGO_BIN_PATH: &str = "cargo run ";
    const root_dir: &str = env!("CARGO_MANIFEST_DIR");

    #[tokio::test]
    async fn default_instance_settings() -> Result<(), Box<dyn std::error::Error>> {
        std::env::set_current_dir(
            PathBuf::from(root_dir)
                .join("tests")
                .join("tomls")
                .join("merge"),
        )?;

        // Path to the overlay.toml file
        let overlay_config_path = PathBuf::from(root_dir)
            .join("tests")
            .join("tomls")
            .join("merge")
            .join("overlay.toml");
        let overlay_config_str = overlay_config_path.to_str().ok_or("Invalid path")?;

        // Running `tembo init`
        let _output = Command::new(CARGO_BIN_PATH).arg("init");

        let _output = Command::new(CARGO_BIN_PATH)
            .arg("apply")
            .arg("--merge")
            .arg(overlay_config_str);

        let merged_settings = apply::get_instance_settings(Some(overlay_config_str.to_string()))?;
        if let Some(setting) = merged_settings.get("defaults") {
            assert_ne!(setting.cpu, "0.25", "Default setting was overwritten");
        } else {
            return Err("Setting key not found".into());
        }

        // Running `tembo delete`
        let _output = Command::new(CARGO_BIN_PATH).arg("delete");

        Ok(())
    }

    #[tokio::test]
    async fn merge() -> Result<(), Box<dyn std::error::Error>> {
        std::env::set_current_dir(
            PathBuf::from(root_dir)
                .join("tests")
                .join("tomls")
                .join("merge"),
        )?;

        // Path to the overlay.toml file
        let overlay_config_path = PathBuf::from(root_dir)
            .join("tests")
            .join("tomls")
            .join("merge")
            .join("overlay.toml");
        let overlay_config_str = overlay_config_path.to_str().ok_or("Invalid path")?;

        // Running `tembo init`
        let _output = Command::new(CARGO_BIN_PATH).arg("init");

        let _output = Command::new(CARGO_BIN_PATH)
            .arg("apply")
            .arg("--merge")
            .arg(overlay_config_str);

        let merged_settings = apply::get_instance_settings(Some(overlay_config_str.to_string()))?;
        if let Some(setting) = merged_settings.get("defaults") {
            assert_eq!(setting.memory, "10Gi", "Base settings was not overwritten");
        } else {
            return Err("Setting key not found".into());
        }

        // Running `tembo delete`
        let _output = Command::new(CARGO_BIN_PATH).arg("delete");

        Ok(())
    }
}
