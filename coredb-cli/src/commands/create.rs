use super::SubCommand;
use clap::Args;

#[derive(Args)]
pub struct CreateCommand {
    resource_type: String,
    name: String,
}

impl SubCommand for CreateCommand {
    fn execute(&self) {
        if self.resource_type == "db" {
            println!("Creating a new db with name: {}", self.name);
        } else if self.resource_type == "extension" {
            println!("Creating a new extension with name: {}", self.name);
        }
    }
}
