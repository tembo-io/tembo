use super::SubCommand;
use crate::commands::pgx::build_pgx;
use clap::Args;
use std::path::Path;
use toml::Table;

#[derive(Args)]
pub struct BuildCommand {
    #[arg(short = 'p', long = "path", default_value = ".")]
    path: String,
    #[arg(short = 'o', long = "output-path", default_value = "./.trunk")]
    output_path: String,
}

impl SubCommand for BuildCommand {
    fn execute(&self) {
        println!("Building from path {}", self.path);
        let path = Path::new(&self.path);
        if path.join("Cargo.toml").exists() {
            let cargo_toml: Table =
                toml::from_str(&std::fs::read_to_string(path.join("Cargo.toml")).unwrap()).unwrap();
            let dependencies = cargo_toml.get("dependencies").unwrap().as_table().unwrap();
            if dependencies.contains_key("pgx") {
                println!("Detected that we are building a pgx extension");
                build_pgx(path, &self.output_path);
                return;
            }
        }
        println!("Did not understand what to build");
    }
}
