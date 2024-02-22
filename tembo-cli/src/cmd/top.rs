use crate::cli::context::{get_current_context, Environment, Profile, Target};
use crate::cli::tembo_config::InstanceSettings;
use crate::cmd::apply::get_instance_settings;
use crate::Args;
use anyhow::anyhow;
use anyhow::{Context, Result};
use hyper::header::ACCEPT;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use temboclient::models::{instance, instance_event};
use temboclient::{
    apis::{
        configuration::Configuration,
        instance_api::{create_instance, get_all, get_instance, put_instance},
    },
    models::connection_info,
};
use tokio::runtime::Runtime;
use tokio::time::{interval, Duration};

#[derive(Args)]
pub struct TopCommand {}

//Format to display the response. Will be changed in beautify.
#[derive(Serialize, Deserialize, Debug)]
pub struct MetricsResponse {
    pub status: String,
    pub data: MetricsData,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MetricsData {
    pub resultType: String,
    pub result: Vec<MetricResult>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MetricResult {
    pub metric: HashMap<String, String>,
    pub value: (u64, String),
}

async fn get_instance_id(
    instance_name: &str,
    config: &Configuration,
    env: &Environment,
) -> Result<Option<String>, anyhow::Error> {
    let v = get_all(config, env.org_id.clone().unwrap().as_str()).await;

    match v {
        Ok(result) => {
            let maybe_instance = result
                .iter()
                .find(|instance| instance.instance_name == instance_name);

            if let Some(instance) = maybe_instance {
                return Ok(Some(instance.clone().instance_id));
            }
        }
        Err(error) => eprintln!("Error getting instance: {}", error),
    };
    Ok(None)
}

async fn fetch_metrics_loop(
    config: &Configuration,
    env: Environment,
    instance_settings: HashMap<String, InstanceSettings>,
) -> Result<()> {
    let client = reqwest::Client::new();

    let mut headers = HeaderMap::new();
    headers.insert("Accept", "application/json".parse()?);
    let jwt_token = config
        .bearer_access_token
        .as_ref()
        .expect("JWT Token is not configured");
    headers.insert("Authorization", format!("Bearer {}", jwt_token).parse()?);
    let mut interval = interval(Duration::from_secs(10));

    loop {
        interval.tick().await;
        for (_key, value) in instance_settings.iter() {
            let org_name = get_instance_org_name(&config, &env, &value.instance_name).await;
            let namespace = format!("org-{}-inst-{}", org_name, &value.instance_name);
            let namespace_encoded = urlencoding::encode(&namespace);
            //4 queries for memory, storage, cpu and Conncections
            let metric_queries = vec![
        format!("sum by (pod) (node_namespace_pod_container:container_cpu_usage_seconds_total:sum_irate{{ namespace=\"{}\", container=\"postgres\" }}) / avg by (pod) (kube_pod_container_resource_limits{{ job=\"kube-state-metrics\", namespace=\"{}\", container=\"postgres\", resource=\"cpu\" }})",namespace_encoded, namespace_encoded).to_string(),
        format!("sum by(persistentvolumeclaim) (kubelet_volume_stats_capacity_bytes{{job=\"kubelet\", metrics_path=\"/metrics\", namespace=\"{}\"}}) - sum by(persistentvolumeclaim) (kubelet_volume_stats_available_bytes{{job=\"kubelet\", metrics_path=\"/metrics\", namespace=\"{}\"}})",namespace_encoded, namespace_encoded).to_string(),
        format!("sum(container_memory_working_set_bytes{{job=\"kubelet\", metrics_path=\"/metrics/cadvisor\", namespace=\"{}\",container!=\"\", image!=\"\"}}) / sum(max by(pod) (kube_pod_container_resource_requests{{job=\"kube-state-metrics\", namespace=\"{}\", resource=\"memory\"}}))",namespace_encoded, namespace_encoded).to_string(),
        format!("max by (pod) (cnpg_backends_max_tx_duration_seconds{{namespace=\"{}\"}})",namespace_encoded).to_string(),
    ];

            for query in &metric_queries {
                //Looping it every 2 seconds to retrieve the response
                match fetch_metric(query, &namespace_encoded, &client, &headers).await {
                    Ok(metrics_response) => println!("{:?}", metrics_response),
                    Err(e) => eprintln!("Error fetching metrics: {}", e),
                }
            }
        }
    }
}

async fn fetch_metric(
    metric_query: &str,
    namespace_encoded: &str,
    client: &reqwest::Client,
    headers: &HeaderMap,
) -> Result<MetricsResponse> {
    const BASE_URL: &str = "https://api.data-1.use1.tembo.io";
    let query_encoded = urlencoding::encode(metric_query);
    let url = format!(
        "{}/{}/metrics/query?&query={}",
        BASE_URL, namespace_encoded, query_encoded
    );

    //Sending the HTTP request with headers
    let response = client
        .get(&url)
        .headers(headers.clone())
        .send()
        .await?
        .json::<MetricsResponse>()
        .await?;

    Ok(response)
}

//Getting the org and instance name to run the queries
async fn get_instance_org_name(
    config: &Configuration,
    env: &Environment,
    instance_name: &String,
) -> String {
    let instance_id = get_instance_id(instance_name, &config, &env)
        .await
        .unwrap()
        .unwrap();
    let instance = get_instance(&config, &env.org_id.clone().unwrap(), &instance_id)
        .await
        .unwrap();
    instance.organization_name
}

//Function to tackle two tokio runtimes
fn blocking(config: &Configuration, env: &Environment) -> Result<(), anyhow::Error> {
    let rt = Runtime::new().unwrap();
    let instance_settings = get_instance_settings(None, None)?;
    rt.block_on(async {
        let _metrics_response = fetch_metrics_loop(&config, env.clone(), instance_settings).await;
    });
    Ok(())
}

pub fn execute() -> Result<(), anyhow::Error> {
    let env = get_current_context().context("Failed to get current context")?;
    let profile = env
        .selected_profile
        .as_ref()
        .context("Expected environment to have a selected profile")?;
    let config = Configuration {
        base_path: profile.tembo_host.clone(),
        bearer_access_token: Some(profile.tembo_access_token.clone()),
        ..Default::default()
    };
    let _result = blocking(&config, &env);
    Ok(())
}
