use super::{ResourceType, SubCommand};
use clap::Args;
use std::process::Command;

#[derive(Args)]
pub struct GetCommand {
    #[arg(value_enum)]
    resource_type: ResourceType,
}

impl SubCommand for GetCommand {
    fn execute(&self) {
        match self.resource_type {
            ResourceType::Db | ResourceType::Dbs => {
                let output = Command::new("kubectl")
                    .arg("get")
                    .arg("tembos")
                    .arg("--all-namespaces")
                    .output()
                    .expect("Failed to execute 'kubectl' command.");
                println!("{}", String::from_utf8_lossy(&output.stdout));
            }
        }
    }
}
