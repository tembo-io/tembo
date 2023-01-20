use super::{ResourceType, SubCommand};
use clap::Args;
use serde_json::json;
use serde_yaml;
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Args)]
pub struct CreateCommand {
    resource_type: ResourceType,
    name: String,
    #[arg(long = "dry-run")]
    dry_run: bool,
}

fn generate_yaml(resource_type: ResourceType, name: String) -> String {
    match resource_type {
        ResourceType::Db | ResourceType::Dbs => {
            let json_value = json!({
                "apiVersion": "kube.rs/v1",
                "kind": "CoreDB",
                "metadata": {
                    "name": name,
                    "namespace": "default",
                },
                "spec": {
                    "replicas": 1,
                },
            });
            serde_yaml::to_string(&json_value).unwrap()
        }
    }
}

impl SubCommand for CreateCommand {
    fn execute(&self) {
        match self.resource_type {
            ResourceType::Db | ResourceType::Dbs => {
                if self.dry_run {
                    println!(
                        "{}",
                        generate_yaml(self.resource_type.clone(), self.name.clone())
                    );
                } else {
                    println!("Creating a new db with name: {}", self.name);

                    let mut kubectl = Command::new("kubectl")
                        .arg("apply")
                        .arg("-f")
                        .arg("-")
                        .stdin(Stdio::piped())
                        .spawn()
                        .expect("Failed to spawn kubectl process");

                    // Write the YAML string to the command's stdin
                    if let Some(ref mut stdin) = kubectl.stdin {
                        stdin
                            .write_all(
                                generate_yaml(self.resource_type.clone(), self.name.clone())
                                    .as_bytes(),
                            )
                            .expect("Failed to write to kubectl stdin");
                    }

                    // Wait for the command to finish and check its exit status
                    let status = kubectl.wait().expect("Failed to wait on kubectl");

                    if !status.success() {
                        eprintln!("kubectl apply failed with status {:?}", status);
                        eprintln!("\n\nHint: Is CoreDB installed in the cluster?\n\nTry running 'coredb install'");
                    }
                }
            }
        }
    }
}
