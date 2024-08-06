use crate::cli::context::get_current_context;
use anyhow::{Context, Result};
use clap::Args;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use temboclient::apis::configuration::Configuration;
use toml::Value;

#[derive(Args)]
pub struct ImportCommand {
    /// Organization ID
    org_id: String,
    /// Instance ID
    instance_id: String,
}

#[derive(Deserialize)]
struct TemboTomlResponse {
    #[serde(rename = "tembo.toml")]
    tembo_toml: String,
}

pub fn execute(import_cmd: ImportCommand) -> Result<()> {
    let env = get_current_context()?;
    let org_id = import_cmd.org_id;
    let instance_id = import_cmd.instance_id;

    // Create the configuration
    let profile = env
        .selected_profile
        .as_ref()
        .with_context(|| "Expected [environment] to have a selected profile")?;
    let config = Configuration {
        base_path: profile.get_tembo_host(),
        bearer_access_token: Some(profile.tembo_access_token.clone()),
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new()?;
    let toml_content = rt.block_on(fetch_toml(&org_id, &instance_id, &config))?;

    // Preprocess the TOML content to fix the trunk_project_version formatting
    let toml_content = preprocess_toml(&toml_content);

    // Parse the TOML content into a generic Value
    let mut toml_value: Value = toml::from_str(&toml_content)
        .context("Failed to parse instance information from TOML")?;

    // Extract the instance name
    let instance_name = toml_value
        .as_table()
        .and_then(|table| table.keys().next().cloned())
        .ok_or_else(|| anyhow::anyhow!("Failed to extract instance name from TOML"))?;

    let toml_path = Path::new("tembo.toml");

    if toml_path.exists() {
        // If tembo.toml exists, append the new instance
        let existing_toml_content = fs::read_to_string(toml_path)?;
        let mut existing_toml_value: Value = toml::from_str(&existing_toml_content)?;
    
        if let Some(existing_table) = existing_toml_value.as_table_mut() {
            if let Some(instance_table) = toml_value.as_table_mut() {
                // Extract the content of the instance and insert it directly
                if let Some((_, instance_data)) = instance_table.iter().next() {
                    existing_table.insert(instance_name.clone(), instance_data.clone());
                }
            }
        }
    
        let new_toml_content = toml::to_string(&existing_toml_value)?;
        fs::write(toml_path, new_toml_content)?;
    } else {
        // If tembo.toml does not exist, create it with only this instance
        let mut file = File::create(toml_path)?;
        let mut new_toml_value = toml::value::Table::new();
        if let Some(instance_table) = toml_value.as_table_mut() {
            if let Some((_, instance_data)) = instance_table.iter().next() {
                new_toml_value.insert(instance_name.clone(), instance_data.clone());
            }
        }
    
        let new_toml_string = toml::to_string(&Value::Table(new_toml_value))?;
        file.write_all(new_toml_string.as_bytes())?;
    }    

    println!("Instance imported successfully.");

    Ok(())
}

async fn fetch_toml(org_id: &str, instance_id: &str, config: &Configuration) -> Result<String> {
    let client = reqwest::Client::new();
    let url = format!(
        "http://localhost:8080/api/v1/orgs/{}/instances/{}/toml",
        org_id, instance_id
    );

    let bearer_token = config
        .bearer_access_token
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Missing bearer access token"))?;

    let response: TemboTomlResponse = client
        .get(&url)
        .header("accept", "application/json")
        .header("Authorization", format!("Bearer {}", bearer_token))
        .send()
        .await
        .context("Failed to send request")?
        .json()
        .await
        .context("Failed to parse JSON response")?;

    Ok(response.tembo_toml)
}

fn preprocess_toml(toml_content: &str) -> String {
    toml_content
        .replace("Some(\"", "")
        .replace("\")", "")
}
