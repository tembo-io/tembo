use super::{ResourceType, SubCommand};
use clap::Args;

#[derive(Args)]
pub struct CreateCommand {
    resource_type: ResourceType,
    name: String,
}

impl SubCommand for CreateCommand {
    fn execute(&self) {
        match self.resource_type {
            ResourceType::Db | ResourceType::Dbs => {
                println!("Creating a new db with name: {}", self.name);
            }
        }
    }
}
