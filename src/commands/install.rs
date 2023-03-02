use super::SubCommand;
use clap::Args;

#[derive(Args)]
pub struct InstallCommand {}

impl SubCommand for InstallCommand {
    fn execute(&self) {
        println!("trunk install: not implemented")
    }
}
