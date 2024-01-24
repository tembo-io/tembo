use super::*;
use anyhow::{Context, Result};
use clap::Args;
use std::process::Command;

#[derive(Args)]
pub struct LogsCommand {
    #[clap(short, long)]
    pub verbose: bool,
}

impl LogsCommand {
    pub fn execute(&self) -> Result<()> {
        let instance_settings = apply::get_instance_settings(None, None)?;

        for (instance_name, _settings) in instance_settings {
            fetch_and_print_docker_logs(&_settings.instance_name)?;
        }

        Ok(())
    }
}

pub fn fetch_and_print_docker_logs(instance_name: &str) -> Result<()> {
    println!("Fetching logs for instance: {}", instance_name);
    let output = Command::new("docker")
        .args(["logs", instance_name])
        .output()
        .with_context(|| {
            format!(
                "Failed to fetch logs for Docker container '{}'",
                instance_name
            )
        })?;

    if !output.status.success() {
        eprintln!("Error fetching logs for instance '{}'", instance_name);
        return Ok(());
    }

    let logs_stdout = String::from_utf8_lossy(&output.stdout);
    let logs_stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{}", logs_stdout);
    println!("STDERR:\n{}", logs_stderr);

    Ok(())
}
