use clap::{Args, Parser, Subcommand};

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
        #[arg(short, long)]
        resource_type: String,
    },
    Create {
        #[arg(short, long)]
        resource_type: String,
        #[arg(short, long)]
        name: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        SubCommands::Get { resource_type} => {
            if resource_type == "dbs" {
                println!("Getting all dbs");
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
