use crate::Result;
use anyhow::bail;
use simplelog::*;
use spinners::{Spinner, Spinners};
use std::process::Command as ShellCommand;
use std::process::Output;

pub struct Docker {}

impl Docker {
    pub fn info() -> Output {
        ShellCommand::new("sh")
            .arg("-c")
            .arg("docker info")
            .output()
            .expect("failed to execute process")
    }

    pub fn installed_and_running() -> Result {
        info!("Checking requirements: [Docker]");

        let output = Self::info();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();

        // determine if docker is installed
        if stdout.is_empty() && !stderr.is_empty() {
            bail!("- Docker is not installed, please visit docker.com to install")
        } else {
            // determine if docker is running
            if !stdout.is_empty() && !stderr.is_empty() {
                let connection_err = stderr.find("Cannot connect to the Docker daemon");

                if connection_err.is_some() {
                    bail!("- Docker is not running, please start it and try again")
                }
            }
        }

        Ok(())
    }

    // Build & run docker image
    pub fn build_run(instance_name: String) -> Result {
        let mut sp = Spinner::new(Spinners::Line, "Running Docker Build & Run".into());

        if Self::container_list_filtered(&instance_name)
            .unwrap()
            .contains(&instance_name)
        {
            sp.stop_with_message("- Existing container found".to_string());
        } else {
            let command = format!(
                "docker build . -t postgres && docker run --name {} -p 5432:5432 -d postgres",
                instance_name
            );
            run_command(&command)?;
            sp.stop_with_message("- Docker Build & Run completed".to_string());
        }

        Ok(())
    }

    // run sqlx migrate
    pub fn run_sqlx_migrate() -> Result {
        let mut sp = Spinner::new(Spinners::Line, "Running SQL migration".into());

        let command = "DATABASE_URL=postgres://postgres:postgres@localhost:5432 sqlx migrate run";
        run_command(command)?;

        sp.stop_with_message("- SQL migration completed".to_string());

        Ok(())
    }

    // stop & remove container for given name
    pub fn stop_remove(name: &str) -> Result {
        let mut sp = Spinner::new(Spinners::Line, "Stopping & Removing instance".into());

        if !Self::container_list_filtered(name).unwrap().contains(name) {
            sp.stop_with_message(format!("- Tembo instance {} doesn't exist", name));
        } else {
            let mut command: String = String::from("docker stop ");
            command.push_str(name);
            command.push_str(" && docker rm ");
            command.push_str(name);

            let output = ShellCommand::new("sh")
                .arg("-c")
                .arg(&command)
                .output()
                .expect("failed to execute process");

            sp.stop_with_message(format!("- Tembo instance {} stopped & removed", &name));

            let stderr = String::from_utf8(output.stderr).unwrap();

            if !stderr.is_empty() {
                bail!("There was an issue stopping the instance: {}", stderr)
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn container_list() -> Result<String> {
        let mut ls_command = String::from("cd tembo "); // TODO: does this work for installed crates?
        ls_command.push_str("&& docker ls --all");

        let output = ShellCommand::new("sh")
            .arg("-c")
            .arg(&ls_command)
            .output()
            .expect("failed to execute process");
        let stdout = String::from_utf8(output.stdout);

        Ok(stdout.unwrap())
    }

    pub fn container_list_filtered(name: &str) -> Result<String> {
        let ls_command = format!("docker container ls --all -f name={}", name);

        let output = ShellCommand::new("sh")
            .arg("-c")
            .arg(&ls_command)
            .output()
            .expect("failed to execute process");
        let stdout = String::from_utf8(output.stdout);

        Ok(stdout.unwrap())
    }
}

pub fn run_command(command: &str) -> Result<()> {
    let output = ShellCommand::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .expect("failed to execute process");

    // Using output status to determine whether there is an error or not
    // because stderr returns a value even when there is no error
    if !output.status.success() {
        let stderr = String::from_utf8(output.stderr).unwrap();
        bail!("There was an issue running command: {}", stderr)
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore] // TODO: implement a mocking library and mock the info function
    fn docker_installed_and_running_test() {
        // without docker installed
        // with docker installed and running
        // with docker installed by not running
    }
}
