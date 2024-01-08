use crate::cli::context::{tembo_context_file_path, tembo_credentials_file_path};
use crate::cli::file_utils::FileUtils;
use crate::cli::tembo_config::InstanceSettings;
use crate::tui::white_confirmation;
use anyhow::Error;
use anyhow::Ok;
use clap::Args;
use std::{collections::HashMap, fs, path::Path, str::FromStr};

/// Validates the tembo.toml file, context file, etc.
#[derive(Args)]
pub struct ValidateCommand {}

pub fn execute(verbose: bool) -> Result<(), anyhow::Error> {
    let mut has_error = false;

    if !Path::new(&tembo_context_file_path()).exists() {
        println!(
            "No {} file exists. Run tembo init first!",
            tembo_context_file_path()
        );
        has_error = true
    }
    if verbose {
        println!("- Context file exists");
    }

    if !Path::new(&tembo_credentials_file_path()).exists() {
        println!(
            "No {} file exists. Run tembo init first!",
            tembo_credentials_file_path()
        );
        has_error = true
    }
    if verbose {
        println!("- Credentials file exists");
    }

    if !Path::new(&"tembo.toml").exists() {
        println!("No Tembo file (tembo.toml) exists in this directory!");
        has_error = true
    } else {
        let mut file_path = FileUtils::get_current_working_dir();
        file_path.push_str("/tembo.toml");

        let contents = fs::read_to_string(file_path.clone())?;
        let config: HashMap<String, InstanceSettings> = toml::from_str(&contents)?;

        // Validate the config
        match validate_config(config, verbose) {
            std::result::Result::Ok(_) => (),
            std::result::Result::Err(e) => {
                println!("Error validating config: {}", e);
                has_error = true;
            }
        }
    }
    if verbose {
        println!("- Tembo file exists");
    }

    if has_error {
        return Err(Error::msg("Fix errors above!"));
    }

    white_confirmation("Configuration is valid");

    Ok(())
}

fn validate_config(
    config: HashMap<String, InstanceSettings>,
    verbose: bool,
) -> Result<(), anyhow::Error> {
    for (section, settings) in config {
        // Validate the environment
        let env_str = settings.environment.as_str();
        validate_environment(env_str, &section, verbose)?;

        // Validate the cpu
        let cpu_str = settings.cpu.as_str();
        validate_cpu(cpu_str, &section, verbose)?;

        // Validate the memory
        let memory_str = settings.memory.as_str();
        validate_memory(memory_str, &section, verbose)?;

        // Validate the storage
        let storage_str = settings.storage.as_str();
        validate_storage(storage_str, &section, verbose)?;

        // Validate the replicas
        let replicas_str = settings.replicas.to_string();
        validate_replicas(&replicas_str, &section, verbose)?;

        // Validate the stack types
        let stack_types_str = settings.stack_type.as_str();
        validate_stack_type(stack_types_str, &section, verbose)?;
    }
    Ok(())
}

fn validate_environment(env: &str, section: &str, verbose: bool) -> Result<(), anyhow::Error> {
    match temboclient::models::Environment::from_str(env) {
        std::result::Result::Ok(_) => {
            if verbose {
                println!("- Environment '{}' in section '{}' is valid", env, section);
            }
            Ok(())
        }
        std::result::Result::Err(_) => Err(Error::msg(format!(
            "Invalid environment setting in section '{}': {}",
            section, env
        ))),
    }
}

fn validate_cpu(cpu: &str, section: &str, verbose: bool) -> Result<(), anyhow::Error> {
    match temboclient::models::Cpu::from_str(cpu) {
        std::result::Result::Ok(_) => {
            if verbose {
                println!("- Cpu '{}' in section '{}' is valid", cpu, section);
            }
            Ok(())
        }
        std::result::Result::Err(_) => Err(Error::msg(format!(
            "Invalid cpu setting in section '{}': {}",
            section, cpu
        ))),
    }
}

fn validate_memory(memory: &str, section: &str, verbose: bool) -> Result<(), anyhow::Error> {
    match temboclient::models::Memory::from_str(memory) {
        std::result::Result::Ok(_) => {
            if verbose {
                println!("- Memory '{}' in section '{}' is valid", memory, section);
            }
            Ok(())
        }
        std::result::Result::Err(_) => Err(Error::msg(format!(
            "Invalid memory setting in section '{}': {}",
            section, memory
        ))),
    }
}

fn validate_storage(storage: &str, section: &str, verbose: bool) -> Result<(), anyhow::Error> {
    match temboclient::models::Storage::from_str(storage) {
        std::result::Result::Ok(_) => {
            if verbose {
                println!("- Storage '{}' in section '{}' is valid", storage, section);
            }
            Ok(())
        }
        std::result::Result::Err(_) => Err(Error::msg(format!(
            "Invalid storage setting in section '{}': {}",
            section, storage
        ))),
    }
}

fn validate_replicas(replicas: &str, section: &str, verbose: bool) -> Result<(), anyhow::Error> {
    match replicas.parse::<u32>() {
        std::result::Result::Ok(value) => {
            if value == 1 || value == 2 {
                if verbose {
                    println!(
                        "- Replicas '{}' in section '{}' is valid",
                        replicas, section
                    );
                }
                Ok(())
            } else {
                Err(Error::msg(format!(
                    "Invalid replicas setting in section '{}': {}. Value must be 1 or 2.",
                    section, replicas
                )))
            }
        }
        Err(_) => Err(Error::msg(format!(
            "Invalid replicas setting in section '{}': {}. Value must be a number.",
            section, replicas
        ))),
    }
}

fn validate_stack_type(
    stack_types: &str,
    section: &str,
    verbose: bool,
) -> Result<(), anyhow::Error> {
    match temboclient::models::StackType::from_str(stack_types) {
        std::result::Result::Ok(_) => {
            if verbose {
                println!(
                    "- Stack types '{}' in section '{}' is valid",
                    stack_types, section
                );
            }
            Ok(())
        }
        std::result::Result::Err(_) => Err(Error::msg(format!(
            "Invalid stack types setting in section '{}': {}",
            section, stack_types
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("prod", true)]
    #[case("dev", true)]
    #[case("test", true)]
    #[case("invalid_env", false)]
    fn test_validate_environment(#[case] env: &str, #[case] is_valid: bool) {
        let result = validate_environment(env, "test_section", false);
        assert_eq!(result.is_ok(), is_valid);
    }

    #[rstest]
    #[case("0.25", true)]
    #[case("0.5", true)]
    #[case("0.75", false)]
    #[case("1", true)]
    #[case("2", true)]
    #[case("4", true)]
    #[case("7", false)]
    fn test_validate_cpu(#[case] cpu: &str, #[case] is_valid: bool) {
        let result = validate_cpu(cpu, "test_section", false);
        assert_eq!(result.is_ok(), is_valid);
    }

    #[rstest]
    #[case("1Gi", true)]
    #[case("2Gi", true)]
    #[case("4Gi", true)]
    #[case("16gi", false)]
    fn test_validate_memory(#[case] memory: &str, #[case] is_valid: bool) {
        let result = validate_memory(memory, "test_section", false);
        assert_eq!(result.is_ok(), is_valid);
    }

    #[rstest]
    #[case("10Gi", true)]
    #[case("50Gi", true)]
    #[case("100Gi", true)]
    #[case("120Gi", false)]
    #[case("200gi", false)]
    fn test_validate_storage(#[case] storage: &str, #[case] is_valid: bool) {
        let result = validate_storage(storage, "test_section", false);
        assert_eq!(result.is_ok(), is_valid);
    }

    #[rstest]
    #[case("1", true)]
    #[case("2", true)]
    #[case("4", false)]
    fn test_validate_replicas(#[case] replicas: &str, #[case] is_valid: bool) {
        let result = validate_replicas(replicas, "test_section", false);
        assert_eq!(result.is_ok(), is_valid);
    }

    #[rstest]
    #[case("Standard", true)]
    #[case("VectorDB", true)]
    #[case("OLTP", true)]
    #[case("OLAP", true)]
    #[case("datawarehouse", false)]
    fn test_validate_stack_type(#[case] stack_type: &str, #[case] is_valid: bool) {
        let result = validate_stack_type(stack_type, "test_section", false);
        assert_eq!(result.is_ok(), is_valid);
    }
}
