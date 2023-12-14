use std::collections::HashMap;

use crate::{
    cli::{
        context::{get_current_context, Environment, Target},
        docker::Docker,
        tembo_config::InstanceSettings,
    },
    Result,
};
use clap::{ArgMatches, Command};
use core::result::Result::Ok;
use temboclient::apis::{configuration::Configuration, instance_api::delete_instance};
use tokio::runtime::Runtime;

use super::apply::{get_instance_id, get_instance_settings};

// Create init subcommand arguments
pub fn make_subcommand() -> Command {
    Command::new("delete").about("Deletes database instance locally & on tembo cloud")
}

pub fn execute(_args: &ArgMatches) -> Result<()> {
    let env = get_current_context()?;

    if env.target == Target::Docker.to_string() {
        Docker::stop_remove("tembo-pg")?;
    } else if env.target == Target::TemboCloud.to_string() {
        return execute_tembo_cloud(env);
    }

    Ok(())
}

fn execute_tembo_cloud(env: Environment) -> Result<()> {
    let instance_settings: HashMap<String, InstanceSettings> = get_instance_settings()?;

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
                Ok(result) => {
                    println!(
                        "Instance delete started for Instance Id: {}",
                        result.instance_id
                    )
                }
                Err(error) => eprintln!("Error deleting instance: {}", error),
            };
        }
    }

    Ok(())
}
