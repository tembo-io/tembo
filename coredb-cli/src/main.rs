use clap::{Args, Parser, Subcommand};
use std::process::Command;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: SubCommands,
}

#[derive(Subcommand)]
enum SubCommands {
    Get {
        resource_type: String,
    },
    Create {
        resource_type: String,
        name: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        SubCommands::Get { resource_type} => {
            if resource_type == "dbs" {
                let output = Command::new("kubectl")
                    .arg("get")
                    .arg("coredbs")
                    .arg("--all-namespaces")
                    .output()
                    .expect("Failed to execute 'kubectl' command.");
                println!("{}", String::from_utf8_lossy(&output.stdout));
            } else if resource_type == "extensions" {
                println!("Getting all extensions");
            }
        }
        SubCommands::Create { resource_type, name} => {
            if resource_type == "db" {
                println!("db with name: {}", name);
            } else if resource_type == "extension" {
                println!("Creating a new extension with name: {}", name);
            }
        }
    }
}
