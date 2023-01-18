use clap::Args;
use super::SubCommand;

#[derive(Args)]
pub struct CreateCommand {
    resource_type: String,
    name: String,
}

impl CreateCommand {
    fn new(resource_type: &str, name: &str) -> Self {
        CreateCommand {
            resource_type: resource_type.to_owned(),
            name: name.to_owned(),
        }
    }
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
