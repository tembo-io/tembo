use crate::cli::context::{get_current_context, Environment, Target};
use crate::cli::docker::Docker;

use crate::tui::confirmation;
use clap::Args;
use core::result::Result::Ok;
use temboclient::apis::{configuration::Configuration, instance_api::delete_instance};
use tokio::runtime::Runtime;

use super::apply::{get_instance_id, get_instance_settings};

/// Deletes database instance locally or on Tembo Cloud
#[derive(Args)]
pub struct DeleteCommand {}

pub fn execute() -> Result<(), anyhow::Error> {
    let env = get_current_context()?;

    let instance_settings = get_instance_settings(None)?;

    if env.target == Target::Docker.to_string() {
        for (_key, value) in instance_settings.iter() {
            Docker::stop_remove(&value.instance_name.clone())?;
        }
    } else if env.target == Target::TemboCloud.to_string() {
        return execute_tembo_cloud(env);
    }

    Ok(())
}

fn execute_tembo_cloud(env: Environment) -> Result<(), anyhow::Error> {
    let instance_settings = get_instance_settings(None)?;

    let profile = env.clone().selected_profile.unwrap();
    let config = Configuration {
        base_path: profile.tembo_host,
        bearer_access_token: Some(profile.tembo_access_token),
        ..Default::default()
    };

    for (_key, value) in instance_settings.iter() {
        let instance_id = get_instance_id(value.instance_name.clone(), &config, env.clone())?;
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
                Err(error) => eprintln!("Error deleting instance: {}", error),
            };
        }
    }

    Ok(())
}
