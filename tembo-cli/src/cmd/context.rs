use clap::{ArgMatches, Args, Subcommand};
use simplelog::*;
use set::ContextSetArgs;

pub mod list;
pub mod set;

/// Manage Tembo contexts
#[derive(Args)]
pub struct ContextCommand {
    #[clap(subcommand)]
    pub subcommand: ContextSubCommand,
}

// Enum for subcommands of 'context'
#[derive(Subcommand)]
pub enum ContextSubCommand {
    /// List all available contexts
    List,
    /// Set the current context
    Set(ContextSetArgs),
}