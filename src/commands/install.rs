use super::SubCommand;
use clap::Args;
use std::process::Command;

#[derive(Args)]
pub struct InstallCommand {
    #[arg(short = 'b', long = "branch", default_value = "main")]
    branch: String,
}

impl SubCommand for InstallCommand {
    fn execute(&self) {
        let output = Command::new("kubectl")
            .arg("apply")
            .arg("-f")
            .arg(
                format!("https://raw.githubusercontent.com/CoreDB-io/coredb/{}/coredb-operator/yaml/crd.yaml", self.branch)
                )
            .output()
            .expect("Failed to execute 'kubectl' command.");
        println!("{}", String::from_utf8_lossy(&output.stdout));
        let output = Command::new("kubectl")
            .arg("apply")
            .arg("-f")
            .arg(
                format!("https://raw.githubusercontent.com/CoreDB-io/coredb/{}/coredb-operator/yaml/install.yaml", self.branch)
            )
            .output()
            .expect("Failed to execute 'kubectl' command.");
        println!("{}", String::from_utf8_lossy(&output.stdout));
    }
}
