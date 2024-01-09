use anyhow::{Result, Context};
use clap::Args;
use crate::cli::file_utils::FileUtils;
use crate::cli::tembo_config::InstanceSettings;
use std::process::Command;
use std::collections::HashMap;
use toml;
use super::*;

#[derive(Args)]
pub struct LogsCommand {
    #[clap(short, long)]
    pub verbose: bool,
}

impl LogsCommand {
    pub fn execute(&self) -> Result<()> {
        let instance_settings = apply::get_instance_settings()?;

        for (instance_name, _) in instance_settings {
            if self.verbose {
                println!("Logs for instance: {}", instance_name);
            }
            Self::fetch_and_print_docker_logs(&instance_name)?;
        }

        Ok(())
    }

    fn fetch_and_print_docker_logs(instance_name: &str) -> Result<()> {
        println!("{}",instance_name);
        let output = Command::new("docker")
            .args(["logs", instance_name])
            .args(["--details"])
            .output()
            .with_context(|| format!("Failed to fetch logs for Docker container '{}'", instance_name))?;

        if !output.status.success() {
            eprintln!("Error fetching logs for instance '{}'", instance_name);
            return Ok(());
        }

        let logs = String::from_utf8_lossy(&output.stdout);
        println!("{}", logs);

        Ok(())
    }
}
