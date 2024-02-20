use crate::{
    cli::context::{get_current_context, Environment, Profile, Target},
};
use crate::Args;
use crate::cli::tembo_config::InstanceSettings;
use crate::apply::get_instance_settings;
use super::apply::get_instance_id;
use temboclient::{
    apis::{
        configuration::Configuration,
        instance_api::{create_instance, get_all, get_instance, put_instance},
    },
};
use std::fmt;
use anyhow::{Context, Result};


#[derive(Args)]
pub struct TopCommand {
}


pub async fn execute() -> Result<(), anyhow::Error> {
    let env = get_current_context().context("Failed to get current context")?;
    let profile = env.selected_profile.as_ref().context("Expected environment to have a selected profile")?;
    let config = Configuration {
        base_path: profile.tembo_host.clone(),
        bearer_access_token: Some(profile.tembo_access_token.clone()),
        ..Default::default()
    };

    let org_id = env.org_id.as_ref().context("Org ID not found")?;

    let instance_id = get_instance_id("set", &config, &env).await;
    match get_instance(&config, org_id, &instance_id).await {
                Ok(instance_details) => println!("Instance Details: {:?}", instance_details),
                Err(e) => println!("Error fetching instance details: {}", e),
            }

    Ok(())
}

