use anyhow::bail;
use anyhow::Error;
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

    pub fn installed_and_running() -> Result<(), anyhow::Error> {
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
    pub fn build_run(instance_name: String) -> Result<(), anyhow::Error> {
        let mut sp = Spinner::new(Spinners::Line, "Running Docker Build & Run".into());
        let container_list = Self::container_list_filtered(&instance_name).unwrap();

        if container_list.contains(&instance_name) {
            let container_port = Docker::get_container_port(container_list)?;

            sp.stop_with_message("- Existing container found".to_string());
            Ok(container_port)
        } else {
            let port = Docker::get_available_port()?;

            let command = format!(
                "docker build . -t postgres && docker run --name {} -p {}:{} -d postgres",
                instance_name, port, port
            );
            run_command(&command)?;
            sp.stop_with_message("- Docker Build & Run completed".to_string());
            Ok(port)
        }
    }

    fn get_available_port() -> Result<i32> {
        let ls_command = "docker ps --format '{{.Ports}}'";

        let output = ShellCommand::new("sh")
            .arg("-c")
            .arg(ls_command)
            .output()
            .expect("failed to execute process");
        let stdout = String::from_utf8(output.stdout)?;

        Docker::get_container_port(stdout)
    }

    fn get_container_port(container_list: String) -> Result<i32> {
        if container_list.contains("5432") {
            if container_list.contains("5433") {
                if container_list.contains("5434") {
                    Err(Error::msg(
                        "None of the ports 5432, 5433, 5434 are available!",
                    ))
                } else {
                    Ok(5434)
                }
            } else {
                Ok(5433)
            }
        } else {
            Ok(5432)
        }
    }

    // stop & remove container for given name
    pub fn stop_remove(name: &str) -> Result<(), anyhow::Error> {
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
    pub fn container_list() -> Result<String, anyhow::Error> {
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

    pub fn container_list_filtered(name: &str) -> Result<String, anyhow::Error> {
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

pub fn run_command(command: &str) -> Result<(), anyhow::Error> {
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
