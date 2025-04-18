use crate::cli::context::{get_current_context, Environment, Profile};
use crate::cli::tembo_config::InstanceSettings;
use crate::cmd::apply::get_instance_settings;
use anyhow::anyhow;
use anyhow::{Context, Result};
use clap::Args;
use crossterm::{
    execute,
    terminal::{Clear, ClearType},
};
use prettytable::{row, Table};
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{stdout, Write};
use tembo_api_client::apis::configuration::Configuration;
use tembo_api_client::apis::instance_api::get_all;
use tembo_api_client::apis::instance_api::get_instance;
use tokio::runtime::Runtime;
use tokio::time::Duration;

///[EXPERIMENTAL] View Metric values of your instances
#[derive(Args)]
pub struct TopCommand {
    #[clap(long)]
    pub tail: bool,
}

//Format to display the response. Will be changed in beautify.
#[derive(Serialize, Deserialize, Debug)]
pub struct MetricsResponse {
    pub status: String,
    pub data: MetricsData,
}

#[allow(non_snake_case)]
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
    let org_id = match env.org_id.as_ref() {
        Some(id) => id,
        None => return Err(anyhow!("Org ID not found")),
    };

    match get_all(config, org_id).await {
        Ok(result) => {
            let maybe_instance = result
                .iter()
                .find(|instance| instance.instance_name == instance_name);
            if let Some(instance) = maybe_instance {
                Ok(Some(instance.clone().instance_id))
            } else {
                Ok(None)
            }
        }
        Err(error) => {
            eprintln!("Error getting instance: {}", error);
            Err(error.into())
        }
    }
}

async fn fetch_metrics_loop(
    config: &Configuration,
    env: Environment,
    instance_settings: HashMap<String, InstanceSettings>,
    profile: &Profile,
    tail: bool,
) -> Result<()> {
    let mut stdout = stdout();
    let client = reqwest::Client::new();
    let url = profile.get_tembo_data_host();
    let mut table = Table::new();
    table.add_row(row!["Instance", "CPU", "Storage", "Memory"]);

    let mut headers = HeaderMap::new();
    headers.insert("Accept", "application/json".parse()?);
    let jwt_token = config
        .bearer_access_token
        .as_ref()
        .expect("JWT Token is not configured");
    headers.insert("Authorization", format!("Bearer {}", jwt_token).parse()?);

    if tail {
        execute!(stdout, Clear(ClearType::All))?;
    }

    for value in instance_settings.values() {
        let namespace = get_instance_namespace(config, &env, &value.instance_name).await?;
        let namespace_encoded = urlencoding::encode(&namespace);

        let mut cpu_value = String::new();
        let mut memory_value = String::new();
        let mut storage_value = String::new();

        let metric_queries = vec![
                (
                    "Cpu",
                    format!("sum by (pod) (node_namespace_pod_container:container_cpu_usage_seconds_total:sum_irate{{ namespace=\"{}\", container=\"postgres\" }}) / avg by (pod) (kube_pod_container_resource_limits{{ job=\"kube-state-metrics\", namespace=\"{}\", container=\"postgres\", resource=\"cpu\" }})*100", namespace_encoded, namespace_encoded),
                    format!(
                        "avg by (pod) (kube_pod_container_resource_limits{{ job=\"kube-state-metrics\", namespace=\"{}\", container=\"postgres\", resource=\"cpu\" }})",
                        namespace_encoded
                    )
                ),
                (
                    "Storage",
                    format!(
                        "(sum by(persistentvolumeclaim) (kubelet_volume_stats_capacity_bytes{{job=\"kubelet\", metrics_path=\"/metrics\", namespace=\"{}\"}}) - sum by(persistentvolumeclaim) (kubelet_volume_stats_available_bytes{{job=\"kubelet\", metrics_path=\"/metrics\", namespace=\"{}\"}})) / 100000000", namespace_encoded, namespace_encoded
                    ),
                    format!(
                        "sum by(persistentvolumeclaim) (kubelet_volume_stats_available_bytes{{job=\"kubelet\", metrics_path=\"/metrics\", namespace=\"{}\"}}) / 1000000000",
                        namespace_encoded
                    )
                ),
                (
                    "Memory",
                    format!("sum(container_memory_working_set_bytes{{job=\"kubelet\", metrics_path=\"/metrics/cadvisor\", namespace=\"{}\",container!=\"\", image!=\"\"}}) / sum(max by(pod) (kube_pod_container_resource_requests{{job=\"kube-state-metrics\", namespace=\"{}\", resource=\"memory\"}})) * 100", namespace_encoded, namespace_encoded),
                    format!(
                        "sum(max by(pod) (kube_pod_container_resource_requests{{job=\"kube-state-metrics\", namespace=\"{}\", resource=\"memory\"}})) / 100000000",
                        namespace_encoded
                    )
                ),
                ];

        for (query_name, query1, query2) in &metric_queries {
            let result1 = fetch_metric(query1, &namespace_encoded, &client, &headers, &url).await;
            let result2 = fetch_metric(query2, &namespace_encoded, &client, &headers, &url).await;

            match (result1, result2) {
                (Ok(metrics_response1), Ok(metrics_response2)) => {
                    let raw_value1: f64 = match metrics_response1.data.result.first() {
                        Some(metric_result) => match metric_result.value.1.parse::<f64>() {
                            Ok(parsed_value) => parsed_value,
                            Err(_) => {
                                eprintln!(
                                    "Error parsing value for {}: defaulting to 0.0",
                                    query_name
                                );
                                0.0
                            }
                        },
                        None => {
                            eprintln!("No result found for {}: defaulting to 0.0", query_name);
                            0.0
                        }
                    };
                    let raw_value2: f64 = match metrics_response2.data.result.first() {
                        Some(metric_result) => match metric_result.value.1.parse::<f64>() {
                            Ok(parsed_value) => parsed_value,
                            Err(_) => {
                                eprintln!(
                                    "Error parsing value for {}: defaulting to 0.0",
                                    query_name
                                );
                                0.0
                            }
                        },
                        None => {
                            eprintln!("No result found for {}: defaulting to 0.0", query_name);
                            0.0
                        }
                    };
                    match *query_name {
                        "Cpu" => cpu_value = format!("{:.2}/{:.2}%", raw_value2, raw_value1),
                        "Memory" => memory_value = format!("{:.2}/{:.2}%", raw_value2, raw_value1),
                        "Storage" => {
                            storage_value = format!("{:.2}/{:.2}%", raw_value2, raw_value1)
                        }
                        _ => (),
                    }
                }
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("Error fetching metrics for {}: {}", query_name, e);
                }
            }
        }
        table.add_row(row![
            value.instance_name,
            cpu_value,
            storage_value,
            memory_value
        ]);
    }
    table.printstd();
    stdout.flush()?;
    Ok(())
}

async fn fetch_metric(
    metric_query: &str,
    namespace_encoded: &str,
    client: &reqwest::Client,
    headers: &HeaderMap,
    url: &str,
) -> Result<MetricsResponse> {
    let base_url: &str = url;
    let query_encoded = urlencoding::encode(metric_query);
    let url = format!(
        "{}/{}/metrics/query?&query={}",
        base_url, namespace_encoded, query_encoded
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

async fn get_instance_namespace(
    config: &Configuration,
    env: &Environment,
    instance_name: &String,
) -> Result<String, anyhow::Error> {
    let instance_id_result = get_instance_id(instance_name, config, env).await;
    let instance_id = match instance_id_result {
        Ok(Some(id)) => id,
        Ok(None) => {
            return Err(anyhow!(
                "Instance ID not found for instance name: {}",
                instance_name
            ))
        }
        Err(e) => return Err(e),
    };
    let org_id = env
        .org_id
        .as_ref()
        .ok_or_else(|| anyhow!("Org ID not found"))?;

    let instance_result = get_instance(config, org_id, &instance_id).await;
    match instance_result {
        Ok(instance) => Ok(instance.namespace),
        Err(e) => Err(e.into()),
    }
}

pub fn execute(verbose: bool, top_command: TopCommand) -> Result<(), anyhow::Error> {
    println!("WARNING! EXPERIMENTAL FEATURE!!");
    super::validate::execute(verbose)?;
    let env = get_current_context().context("Failed to get current context")?;
    let profile = env
        .selected_profile
        .as_ref()
        .context("Expected environment to have a selected profile")?;
    let config = Configuration {
        base_path: profile.get_tembo_host(),
        bearer_access_token: Some(profile.tembo_access_token.clone()),
        ..Default::default()
    };
    let instance_settings = get_instance_settings(None, None)?;

    //Looking for --tail here
    if top_command.tail {
        let rt = Runtime::new().map_err(|e| anyhow!("Failed to create Tokio runtime: {}", e))?;
        rt.block_on(async {
            loop {
                if let Err(e) = fetch_metrics_loop(
                    &config,
                    env.clone(),
                    instance_settings.clone(),
                    profile,
                    true,
                )
                .await
                {
                    eprintln!("Error fetching metrics: {}", e);
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });
    } else {
        let rt = Runtime::new().map_err(|e| anyhow!("Failed to create Tokio runtime: {}", e))?;
        rt.block_on(async {
            if let Err(e) = fetch_metrics_loop(
                &config,
                env.clone(),
                instance_settings.clone(),
                profile,
                false,
            )
            .await
            {
                eprintln!("Error fetching metrics: {}", e);
            }
        });
    }

    Ok(())
}
