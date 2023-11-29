use crate::cli::instance::Instance;
use simplelog::*;
use spinners::{Spinner, Spinners};
use std::error::Error;
use std::fmt;
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

    pub fn installed_and_running() -> Result<(), Box<dyn Error>> {
        info!("Checking requirements: [Docker]");

        let output = Self::info();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();

        // determine if docker is installed
        if stdout.is_empty() && !stderr.is_empty() {
            return Err(Box::new(DockerError::new(
                "- Docker is not installed, please visit docker.com to install",
            )));
        } else {
            // determine if docker is running
            if !stdout.is_empty() && !stderr.is_empty() {
                let connection_err = stderr.find("Cannot connect to the Docker daemon");

                if connection_err.is_some() {
                    return Err(Box::new(DockerError::new(
                        "- Docker is not running, please start it and try again",
                    )));
                }
            }
        }

        Ok(())
    }

    // Build & run docker image
    pub fn build_run() -> Result<(), Box<dyn Error>> {
        let container_name = "tembo-pg";

        if Self::container_list_filtered(container_name)
            .unwrap()
            .contains(container_name)
        {
            info!("existing container found");
        } else {
            let command = format!(
                "docker build . -t postgres && docker run --name {} -p 5432:5432 -d postgres",
                container_name
            );
            run_command(&command)?;
        }

        Ok(())
    }

    // run sqlx migrate
    pub fn run_sqlx_migrate() -> Result<(), Box<dyn Error>> {
        let command = "DATABASE_URL=postgres://postgres:postgres@localhost:5432 sqlx migrate run";
        run_command(&command)?;

        Ok(())
    }

    // start container if exists for name otherwise build container and start
    pub fn start(name: &str, instance: &Instance) -> Result<(), Box<dyn Error>> {
        if Self::container_list_filtered(name)
            .unwrap()
            .contains("tembo-pg")
        {
            info!("existing container found");

            instance.start();
        } else {
            info!("building and then running container");

            let _ = instance.init();
        };

        Ok(())
    }

    // stop container for given name
    pub fn stop(name: &str) -> Result<(), Box<dyn Error>> {
        let mut sp = Spinner::new(Spinners::Line, "Stopping instance".into());
        let mut command = String::from("cd tembo ");
        command.push_str("&& docker stop ");
        command.push_str(name);

        let output = ShellCommand::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .expect("failed to execute process");

        let message = format!("- Tembo instance {} stopped", &name);
        sp.stop_with_message(message);

        let stderr = String::from_utf8(output.stderr).unwrap();

        if !stderr.is_empty() {
            return Err(Box::new(DockerError::new(
                format!("There was an issue stopping the instance: {}", stderr).as_str(),
            )));
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn container_list() -> Result<String, Box<dyn Error>> {
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

    pub fn container_list_filtered(name: &str) -> Result<String, Box<dyn Error>> {
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

pub fn run_command(command: &str) -> Result<(), Box<dyn Error>> {
    let mut sp = Spinner::new(Spinners::Line, "Running Docker Build & Run".into());

    let output = ShellCommand::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .expect("failed to execute process");

    let stderr = String::from_utf8(output.stderr).unwrap();

    if !stderr.is_empty() {
        return Err(Box::new(DockerError::new(
            format!("There was an issue building & running docker: {}", stderr).as_str(),
        )));
    }

    Ok(())
}

// Define Docker not installed Error
#[derive(Debug)]
pub struct DockerError {
    details: String,
}

impl DockerError {
    pub fn new(msg: &str) -> DockerError {
        DockerError {
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for DockerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for DockerError {
    fn description(&self) -> &str {
        &self.details
    }
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
