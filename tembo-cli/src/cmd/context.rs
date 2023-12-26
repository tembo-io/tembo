use clap::{ArgMatches, Args, Subcommand};
use simplelog::*;

pub mod list;
pub mod set;

// Subcommand for 'context'
#[derive(Args)]
pub struct ContextCommand {
    #[clap(subcommand)]
    pub subcommand: ContextSubCommand,
}

// Enum for subcommands of 'context'
#[derive(Subcommand)]
pub enum ContextSubCommand {
    List,
    Set(ContextSetArgs),
}

// Arguments for 'context set'
#[derive(Args)]
pub struct ContextSetArgs {
    #[clap(short, long)]
    pub name: String,
}

// handles all context command calls
pub fn execute(args: &ArgMatches) -> Result<(), anyhow::Error> {
    // execute the context subcommands
    let res = match args.subcommand() {
        Some(("list", sub_matches)) => list::execute(sub_matches),
        Some(("set", sub_matches)) => set::execute(sub_matches),

        _ => unreachable!(),
    };

    if res.is_err() {
        error!("{}", res.err().unwrap());

        // TODO: adding logging, log error
        std::process::exit(101);
    }

    Ok(())
}
