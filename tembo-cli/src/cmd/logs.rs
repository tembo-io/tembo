use crate::apply::{get_instance_id, get_instance_settings}; // Adjust the path as needed
use crate::cli::context::{get_current_context, Environment, Profile};
use anyhow::Result;
use clap::Args;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use temboclient::apis::configuration::Configuration;

#[derive(Args)]
pub struct LogsCommand {
    #[clap(short, long)]
    pub verbose: bool,
}

pub fn execute(verbose: bool) -> Result<()> {
    let env = get_current_context()?;
    let org_id = env.org_id.clone().unwrap_or_default();
    let profile = env.selected_profile.clone().unwrap();

    let config = Configuration {
        base_path: profile.tembo_host,
        bearer_access_token: Some(profile.tembo_access_token.clone()),
        ..Default::default()
    };

    let instance_settings = get_instance_settings(None)?;

    let client = Client::new();
    let mut headers = HeaderMap::new();
    headers.insert("X-Scope-OrgID", HeaderValue::from_str(&org_id)?);
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", profile.tembo_access_token))?,
    );

    for (_key, value) in instance_settings.iter() {
        let instance_id_option =
            get_instance_id(value.instance_name.clone(), &config, env.clone())?;

        let instance_id = if let Some(id) = instance_id_option {
            id
        } else {
            eprintln!("Instance ID not found for {}", value.instance_name);
            continue;
        };

        let query = format!("{{tembo_instance_id=\"{}\"}}", instance_id);
        let url = "https://api.data-1.use1.tembo.io/loki/api/v1/query_range";

        let response = client
            .get(url)
            .headers(headers.clone())
            .query(&[("query", &query)])
            .send()?;

        if response.status().is_success() {
            let response_body = response.text()?;
            println!("{}", response_body);
        } else {
            eprintln!("Error: {:?}", response.status());
        }
    }

    Ok(())
}
