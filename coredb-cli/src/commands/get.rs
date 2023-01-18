use super::SubCommand;
use clap::Args;
use std::process::Command;

#[derive(Args)]
pub struct GetCommand {
    #[arg(value_enum)]
    resource_type: ResourceType,
}

#[derive(clap::ValueEnum, Clone)]
enum ResourceType {
    Dbs,
}

impl SubCommand for GetCommand {
    fn execute(&self) {
        match self.resource_type {
            ResourceType::Dbs => {
                let output = Command::new("kubectl")
                    .arg("get")
                    .arg("coredbs")
                    .arg("--all-namespaces")
                    .output()
                    .expect("Failed to execute 'kubectl' command.");
                println!("{}", String::from_utf8_lossy(&output.stdout));
            }
        }
    }
}
