use super::SubCommand;
use async_trait::async_trait;
use clap::Args;
use std::path::PathBuf;
use tokio_task_manager::Task;

#[derive(Args)]
pub struct InstallCommand {
    #[arg(long = "pg-config", short = 'p')]
    pg_config: Option<PathBuf>,
    #[arg(long = "file", short = 'f')]
    file: Option<PathBuf>,
}

#[async_trait]
impl SubCommand for InstallCommand {
    async fn execute(&self, _task: Task) -> Result<(), anyhow::Error> {
        let installed_pg_config = which::which("pg_config").ok();
        let pg_config = self
            .pg_config
            .as_ref()
            .or_else(|| installed_pg_config.as_ref())
            .ok_or(anyhow::Error::msg("pg_config not found"))?;
        println!("Using pg_config: {}", pg_config.to_string_lossy());

        // check if self.file is a path that exists to a file
        if let Some(ref file) = self.file {
            if !file.exists() {
                return Err(anyhow::Error::msg(format!(
                    "{} does not exist",
                    file.display()
                )));
            }
        }

        let package_lib_dir = std::process::Command::new(pg_config)
            .arg("--pkglibdir")
            .output()?
            .stdout;
        let package_lib_dir = String::from_utf8_lossy(&package_lib_dir)
            .trim_end()
            .to_string();
        let package_lib_dir_path = std::path::PathBuf::from(&package_lib_dir);
        let package_lib_dir = std::fs::canonicalize(&package_lib_dir_path)?;

        let sharedir = std::process::Command::new(pg_config.clone())
            .arg("--sharedir")
            .output()?
            .stdout;

        let sharedir = String::from_utf8_lossy(&sharedir).trim_end().to_string();
        let sharedir_path = std::path::PathBuf::from(&sharedir).join("extension");

        let sharedir = std::fs::canonicalize(sharedir_path)?;
        // if this is a symlink, then resolve the symlink

        if !package_lib_dir.exists() && !package_lib_dir.is_dir() {
            println!(
                "The package lib dir {} does not exist",
                package_lib_dir.display()
            );
            return Ok(());
        }
        if !sharedir.exists() && !sharedir.is_dir() {
            println!("The share dir {} does not exist", sharedir.display());
            return Ok(());
        }
        println!("Using pkglibdir: {:?}", package_lib_dir);
        println!("Using sharedir: {:?}", sharedir);
        Ok(())
    }
}
