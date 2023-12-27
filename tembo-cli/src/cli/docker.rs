use anyhow::Error;
use anyhow::{bail, Context};
use simplelog::*;
use spinners::{Spinner, Spinners};
use std::io::{BufRead, BufReader};
use std::process::Output;
use std::process::{Command as ShellCommand, Stdio};
use std::thread;

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

    pub fn build_run(instance_name: String, verbose: bool) -> Result<i32, anyhow::Error> {
        let mut sp = if !verbose {
            Some(Spinner::new(Spinners::Line, "Running Docker Build & Run".into()))
        } else {
            None
        };

        let mut show_message = |message: &str, new_spinner: bool| {
            if let Some(mut spinner) = sp.take() {
                spinner.stop_with_message(message.to_string());
                if new_spinner {
                    sp = Some(Spinner::new(Spinners::Line, "Building and running container".into()));
                }
            } else {
                println!("{}", message);
            }
        };

        let container_list = Self::container_list_filtered(&instance_name)?;

        if container_list.contains(&instance_name) {
            show_message("- Existing container found, removing", true);
            Docker::stop_remove(&instance_name)?;
        }

        let port = Docker::get_available_port()?;

        let command = format!(
            "docker build . -t postgres-{} && docker run --rm --name {} -p {}:{} -d postgres-{}",
            instance_name, instance_name, port, port, instance_name
        );
        run_command(&command, verbose)?;

        show_message("- Docker Build & Run completed", false);

        Ok(port)
    }

    fn get_available_port() -> Result<i32, anyhow::Error> {
        let ls_command = "docker ps --format '{{.Ports}}'";

        let output = ShellCommand::new("sh")
            .arg("-c")
            .arg(ls_command)
            .output()
            .expect("failed to execute process");
        let stdout = String::from_utf8(output.stdout)?;

        Docker::get_container_port(stdout)
    }

    fn get_container_port(container_list: String) -> Result<i32, anyhow::Error> {
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
            let mut command: String = String::from("docker rm --force ");
            command.push_str(name);

            let output = match ShellCommand::new("sh").arg("-c").arg(&command).output() {
                Ok(output) => output,
                Err(_) => {
                    sp.stop_with_message(format!(
                        "- Tembo instance {} failed to stop & remove",
                        &name
                    ));
                    bail!("There was an issue stopping the instance")
                }
            };

            sp.stop_with_message(format!("- Tembo instance {} stopped & removed", &name));

            let stderr = String::from_utf8(output.stderr).unwrap();

            if !stderr.is_empty() {
                bail!("There was an issue stopping the instance: {}", stderr)
            }
        }

        Ok(())
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

pub fn run_command(command: &str, verbose: bool) -> Result<(), anyhow::Error> {
    let mut child = ShellCommand::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn command '{}'", command))?;

    if verbose {
        let stdout = BufReader::new(child.stdout.take().expect("Failed to open stdout"));
        let stderr = BufReader::new(child.stderr.take().expect("Failed to open stderr"));

        let stdout_handle = thread::spawn(move || {
            for line in stdout.lines() {
                match line {
                    Ok(line) => println!("{}", line),
                    Err(e) => eprintln!("Error reading line from stdout: {}", e),
                }
            }
        });

        let stderr_handle = thread::spawn(move || {
            for line in stderr.lines() {
                match line {
                    Ok(line) => eprintln!("{}", line),
                    Err(e) => eprintln!("Error reading line from stderr: {}", e),
                }
            }
        });

        stdout_handle.join().expect("Stdout thread panicked");
        stderr_handle.join().expect("Stderr thread panicked");
    }

    let status = child.wait().expect("Failed to wait on child");

    if !status.success() {
        bail!("Command executed with failures");
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
