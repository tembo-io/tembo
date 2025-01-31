use crate::cli::context::{get_current_context, Environment, Target};
use crate::cli::docker::Docker;
use crate::tui;
use crate::tui::confirmation;
use clap::Args;
use core::result::Result::Ok;
use tembo_api_client::apis::{configuration::Configuration, instance_api::delete_instance};
use tokio::runtime::Runtime;

use super::apply::{get_instance_id, get_instance_settings};

/// Deletes database instance locally or on Tembo Cloud
#[derive(Args)]
pub struct DeleteCommand {}

pub fn execute() -> Result<(), anyhow::Error> {
    let env = get_current_context()?;

    if env.target == Target::Docker.to_string() {
        return Docker::docker_compose_down(true);
    } else if env.target == Target::TemboCloud.to_string() {
        return execute_tembo_cloud(env);
    }

    Ok(())
}

fn execute_tembo_cloud(env: Environment) -> Result<(), anyhow::Error> {
    let instance_settings = get_instance_settings(None, None)?;

    let profile = env.clone().selected_profile.unwrap();
    let config = Configuration {
        base_path: profile.get_tembo_host(),
        bearer_access_token: Some(profile.tembo_access_token),
        ..Default::default()
    };

    for (_key, value) in instance_settings.iter() {
        let instance_id = get_instance_id(&value.instance_name, &config, &env)?;
        if let Some(env_instance_id) = instance_id {
            let v = Runtime::new().unwrap().block_on(delete_instance(
                &config,
                env.clone().org_id.unwrap().as_str(),
                &env_instance_id,
            ));

            match v {
                Ok(result) => confirmation(&format!(
                    "Instance delete started for Instance Id: {}",
                    result.instance_id
                )),
                Err(error) => tui::error(&format!("Error deleting instance: {}", error)),
            };
        } else {
            tui::error(&format!(
                "No instance with name {} found",
                &value.instance_name
            ));
        }
    }

    Ok(())
}
