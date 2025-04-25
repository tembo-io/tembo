use crate::tui::{self, colors, white_confirmation};
use anyhow::{bail, Context, Error};
use colorful::{Color, Colorful};
use simplelog::*;
use spinoff::{spinners, Spinner};
use std::io::{BufRead, BufReader};
use std::path::Path;
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

    pub fn build(instance_name: String, verbose: bool) -> Result<(), anyhow::Error> {
        let mut sp = if !verbose {
            Some(Spinner::new(
                spinners::Dots,
                "Running Docker Build",
                spinoff::Color::White,
            ))
        } else {
            None
        };

        let mut show_message = |message: &str, new_spinner: bool| {
            if let Some(mut spinner) = sp.take() {
                spinner.stop_with_message(&format!(
                    "{} {}",
                    "✓".color(colors::indicator_good()).bold(),
                    message.color(Color::White).bold()
                ));
                if new_spinner {
                    sp = Some(Spinner::new(
                        spinners::Dots,
                        format!("Building container for {}", instance_name),
                        spinoff::Color::White,
                    ));
                }
            } else {
                white_confirmation(message);
            }
        };

        let command = format!(
            "cd {} && docker build . -t postgres-{}",
            instance_name, instance_name
        );
        run_command(&command, verbose)?;

        show_message(
            &format!("Docker Build completed for {}", instance_name),
            false,
        );

        Ok(())
    }

    pub fn docker_compose_up(verbose: bool) -> Result<(), anyhow::Error> {
        let mut sp = if !verbose {
            Some(Spinner::new(
                spinners::Dots,
                "Running Docker Compose Up",
                spinoff::Color::White,
            ))
        } else {
            None
        };

        let mut show_message = |message: &str, new_spinner: bool| {
            if let Some(mut spinner) = sp.take() {
                spinner.stop_with_message(&format!(
                    "{} {}",
                    "✓".color(colors::indicator_good()).bold(),
                    message.color(Color::White).bold()
                ));
                if new_spinner {
                    sp = Some(Spinner::new(
                        spinners::Dots,
                        "Running docker compose up",
                        spinoff::Color::White,
                    ));
                }
            } else {
                white_confirmation(message);
            }
        };

        let command = "docker compose up -d --build";

        if verbose {
            run_command(command, verbose)?;
        } else {
            let output = match ShellCommand::new("sh").arg("-c").arg(command).output() {
                Ok(output) => output,
                Err(err) => {
                    return Err(Error::msg(format!("Issue starting the instances: {}", err)))
                }
            };
            let stderr = String::from_utf8(output.stderr).unwrap();

            if !output.status.success() {
                tui::error(&format!(
                    "\nThere was an issue starting the instances: {}",
                    stderr
                ));

                return Err(Error::msg("Error running docker compose up!"));
            }
        }

        show_message("Docker Compose Up completed", false);

        Ok(())
    }

    pub fn docker_compose_down(verbose: bool) -> Result<(), anyhow::Error> {
        let path: &Path = Path::new("docker-compose.yml");
        if !path.exists() {
            if verbose {
                println!(
                    "{} {}",
                    "✓".color(colors::indicator_good()).bold(),
                    "No docker-compose.yml found in the directory"
                        .color(Color::White)
                        .bold()
                )
            }
            return Ok(());
        }

        let mut sp = Spinner::new(
            spinners::Dots,
            "Running Docker Compose Down",
            spinoff::Color::White,
        );

        let command: String = String::from("docker compose down");

        let output = match ShellCommand::new("sh").arg("-c").arg(&command).output() {
            Ok(output) => output,
            Err(_) => {
                sp.stop_with_message("- Tembo instances failed to stop & remove");
                bail!("There was an issue stopping the instances")
            }
        };

        sp.stop_with_message(&format!(
            "{} {}",
            "✓".color(colors::indicator_good()).bold(),
            "Tembo instances stopped & removed"
                .color(Color::White)
                .bold()
        ));

        let stderr = String::from_utf8(output.stderr).unwrap();

        if !output.status.success() {
            tui::error(&format!(
                "\nThere was an issue stopping the instances: {}",
                stderr
            ));

            return Err(Error::msg("Error running docker compose down!"));
        }

        Ok(())
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
        return Err(Error::msg("Command executed with failures!"));
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
