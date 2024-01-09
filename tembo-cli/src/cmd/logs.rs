use anyhow::{Result, Context};
use clap::Args;
use std::process::Command;
use super::*;

#[derive(Args)]
pub struct LogsCommand {
    #[clap(short, long)]
    pub verbose: bool,
}

impl LogsCommand {
    pub fn execute(&self) -> Result<()> {
        let instance_settings = super::apply::get_instance_settings()?;

        for (instance_name, _settings) in instance_settings {
            let output = Command::new("docker")
                .arg("logs")
                .arg(&instance_name)
                .output()?;

            if output.status.success() {
                println!("Huge success haha");
            } else {
                eprintln!("Failed to get logs for instance: {}", instance_name);
                eprintln!("{}", String::from_utf8_lossy(&output.stderr));
            }
            fetch_and_print_docker_logs(&instance_name)?;
        }

        Ok(())
    }
}


pub fn fetch_and_print_docker_logs(instance_name: &str) -> Result<()> {
    println!("Fetching logs for instance: {}", instance_name);
    let output = Command::new("docker")
        .args(["logs", instance_name])
        .output()
        .with_context(|| format!("Failed to fetch logs for Docker container '{}'", instance_name))?;

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