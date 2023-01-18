use super::SubCommand;
use clap::Args;
use std::process::Command;

#[derive(Args)]
pub struct GetCommand {
    resource_type: String,
}

impl SubCommand for GetCommand {
    fn execute(&self) {
        if self.resource_type == "dbs" {
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
