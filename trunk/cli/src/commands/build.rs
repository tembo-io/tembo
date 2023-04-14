use super::SubCommand;
use crate::commands::pgx::build_pgx;
// use crate::commands::makefile::build_makefile;
use async_trait::async_trait;
use clap::Args;
use std::path::Path;
use anyhow::anyhow;
use tokio_task_manager::Task;
use toml::Table;

#[derive(Args)]
pub struct BuildCommand {
    #[arg(short = 'p', long = "path", default_value = ".")]
    path: String,
    #[arg(short = 'o', long = "output-path", default_value = "./.trunk")]
    output_path: String,
    #[arg(long = "version")]
    version: Option<String>,
    #[arg(long = "name")]
    name: Option<String>,
}

#[async_trait]
impl SubCommand for BuildCommand {
    async fn execute(&self, task: Task) -> Result<(), anyhow::Error> {
        println!("Building from path {}", self.path);
        let path = Path::new(&self.path);
        if path.join("Cargo.toml").exists() {
            let cargo_toml: Table =
                toml::from_str(&std::fs::read_to_string(path.join("Cargo.toml")).unwrap()).unwrap();
            let dependencies = cargo_toml.get("dependencies").unwrap().as_table().unwrap();
            if dependencies.contains_key("pgx") {
                println!("Detected that we are building a pgx extension");
                if self.version.is_some() || self.name.is_some() {
                    return Err(anyhow!("--version and --name are collected from Cargo.toml when building pgx extensions, please do not configure"));
                }
                build_pgx(path, &self.output_path, cargo_toml, task).await?;
                return Ok(());
            }
        }

        // Check for Makefile
        if path.join("Makefile").exists() {
            println!("Detected a Makefile, guessing that we are building a C extension with 'make', 'make install...'");
            // Check if version or name are missing
            if self.version.is_none() || self.name.is_none() {
                println!("Error: --version and --name are required when building a makefile based extension");
                return Err(anyhow!("--version and --name are required when building a makefile based extension"));
            }
            // build_c_extension(path, &self.output_path, task).await?;
            return Ok(());
        }
        println!("Did not understand what to build");
        Ok(())
    }
}
