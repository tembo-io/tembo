// Objects representing a user created local instance of a stack
// (a local container that runs with certain attributes and properties)

use crate::cli::docker::DockerError;
use crate::cli::extension::Extension;
use crate::cli::stacks;
use crate::cli::stacks::{Stack, TrunkInstall};
use chrono::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use spinners::{Spinner, Spinners};
use std::cmp::PartialEq;
use std::error::Error;
use std::process::Command as ShellCommand;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Instance {
    pub name: Option<String>,
    pub r#type: Option<String>,
    pub port: Option<String>, // TODO: persist as an <u16>
    pub version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub installed_extensions: Vec<InstalledExtension>,
    pub enabled_extensions: Vec<EnabledExtension>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledExtension {
    pub name: Option<String>,
    pub version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnabledExtension {
    pub name: Option<String>,
    pub version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub locations: Vec<ExtensionLocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionLocation {
    pub database: String,
    pub enabled: String,
    pub version: String,
}

impl Instance {
    pub fn init(&self) -> Result<(), Box<dyn Error>> {
        let stack = self.stack();

        self.build();

        for install in &stack.trunk_installs {
            let _ = self.install_extension(install);
        }

        for extension in &stack.extensions {
            let _ = self.enable_extension(extension);
        }

        Ok(())
    }

    // Returns the stack the instance is based on
    // TODO: determine if there is a way to return a vector element in a better way
    fn stack(&self) -> Stack {
        let stacks = stacks::define_stacks();
        let stack_type = self.r#type.clone().unwrap().to_lowercase();

        let stack_details: Vec<_> = stacks
            .stacks
            .into_iter()
            .filter(|s| s.name.to_lowercase() == stack_type)
            .collect();

        let stack = stack_details.first().unwrap();

        Stack {
            name: stack.name.clone(),
            description: stack.description.clone(),
            version: stack.version.clone(),
            trunk_installs: stack.trunk_installs.clone(),
            extensions: stack.extensions.clone(),
        }
    }

    // builds (and starts) a new container
    fn build(&self) {
        let port_option = format!(
            "--publish {}:{}",
            self.port.clone().unwrap(),
            self.port.clone().unwrap(),
        );

        let mut command = String::from("cd tembo ");
        command.push_str("&& docker run -d --name ");
        command.push_str(&self.name.clone().unwrap());
        command.push(' ');
        command.push_str(&port_option);
        command.push_str(" tembo-pg");

        let _ = self.run_command(&command);
    }

    // starts the existing container
    pub fn start(&self) {
        let mut command = String::from("cd tembo ");
        command.push_str("&& docker start ");
        command.push_str(&self.name.clone().unwrap());

        let _ = self.run_command(&command);
    }

    fn run_command(&self, command: &str) -> Result<(), Box<dyn Error>> {
        let mut sp = Spinner::new(Spinners::Line, "Starting instance".into());

        let output = ShellCommand::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .expect("failed to execute process");

        let message = format!(
            "- Tembo instance started on {}",
            &self.port.clone().unwrap(),
        );
        sp.stop_with_message(message);

        let stderr = String::from_utf8(output.stderr).unwrap();

        if !stderr.is_empty() {
            return Err(Box::new(DockerError::new(
                format!("There was an issue starting the instance: {}", stderr).as_str(),
            )));
        }

        Ok(())
    }

    fn install_extension(&self, extension: &TrunkInstall) -> Result<(), Box<dyn Error>> {
        let mut sp = Spinner::new(Spinners::Dots12, "Installing extension".into());

        let mut command = String::from("cd tembo && docker exec ");
        command.push_str(&self.name.clone().unwrap());
        command.push_str(" sh -c 'trunk install ");
        command.push_str(&extension.name.clone().unwrap());
        command.push('\'');

        let output = ShellCommand::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .expect("failed to execute process");

        let mut msg = String::from("- Stack extension installed: ");
        msg.push_str(&extension.name.clone().unwrap());

        sp.stop_with_message(msg);

        let stderr = String::from_utf8(output.stderr).unwrap();

        if !stderr.is_empty() {
            return Err(Box::new(DockerError::new(
                format!("There was an issue installing the extension: {}", stderr).as_str(),
            )));
        } else {
            Ok(())
        }
    }

    fn enable_extension(&self, extension: &Extension) -> Result<(), Box<dyn Error>> {
        let mut sp = Spinner::new(Spinners::Dots12, "Enabling extension".into());

        let locations = extension
            .locations
            .iter()
            .map(|s| s.database.as_str())
            .collect::<Vec<&str>>()
            .join(", ");

        let mut command = String::from("docker exec ");
        command.push_str(&self.name.clone().unwrap());
        command.push_str(" sh -c 'psql -U postgres -c create extension if not exists \"");
        command.push_str(&extension.name.clone().unwrap());
        command.push_str("\" schema ");
        command.push_str(&locations);
        command.push_str(" cascade;'");

        let output = ShellCommand::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .expect("failed to execute process");

        let mut msg = String::from("- Stack extension enabled: ");
        msg.push_str(&extension.name.clone().unwrap());

        sp.stop_with_message(msg);

        let stderr = String::from_utf8(output.stderr).unwrap();

        if !stderr.is_empty() {
            return Err(Box::new(DockerError::new(
                format!("There was an issue enabling the extension: {}", stderr).as_str(),
            )));
        } else {
            Ok(())
        }
    }
}
