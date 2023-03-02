use super::SubCommand;
use clap::Args;

#[derive(Args)]
pub struct PublishCommand {}

impl SubCommand for PublishCommand {
    fn execute(&self) {
        println!("trunk publish: not implemented")
    }
}
