use super::SubCommand;
use clap::Args;

#[derive(Args)]
pub struct BuildCommand {}

impl SubCommand for BuildCommand {
    fn execute(&self) {
        println!("trunk build: not implemented")
    }
}
