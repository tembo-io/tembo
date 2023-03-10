use super::SubCommand;
use clap::Args;
use std::process::Command;

#[derive(Args)]
pub struct InstallCommand {}

impl SubCommand for InstallCommand {
    fn execute(&self) {
        let output = Command::new("kubectl")
            .arg("apply")
            .arg("-f")
            .arg("https://raw.githubusercontent.com/CoreDB-io/coredb/release/2023.3.9/coredb-operator/yaml/crd.yaml")
            .output()
            .expect("Failed to execute 'kubectl' command.");
        println!("{}", String::from_utf8_lossy(&output.stdout));
        let output = Command::new("kubectl")
            .arg("apply")
            .arg("-f")
            .arg("https://raw.githubusercontent.com/CoreDB-io/coredb/release/2023.3.9/coredb-operator/yaml/install.yaml")
            .output()
            .expect("Failed to execute 'kubectl' command.");
        println!("{}", String::from_utf8_lossy(&output.stdout));
    }
}
