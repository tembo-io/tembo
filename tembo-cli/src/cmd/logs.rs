use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct LogsCommand {
    #[clap(short, long)]
    pub verbose: bool,
}


impl LogsCommand {
    pub async fn execute(&self) -> Result<()> {
        let org_id = "****";
        let instance_id = "****";
        let token = "****"; // Replace with your actual JWT token

        let client = reqwest::Client::new();
        let mut headers = HeaderMap::new();
        headers.insert("X-Scope-OrgID", HeaderValue::from_str(org_id)?);
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", token))?);

        let query = format!("{{tembo_instance_id=\"{}\"}}", instance_id);
        let url = "https://api.data-1.use1.tembo.io/loki/api/v1/query_range";

        let response = client
            .get(url)
            .headers(headers)
            .query(&[("query", &query)])
            .send()
            .await?;

        if response.status().is_success() {
            let response_body = response.text().await?;

            println!("{}",response_body);

            /*for stream in parsed_response.data.result {
                println!("Stream: {:?}", stream.stream);
                for value in stream.values {
                    println!("Time: {}", value[0]);
                    println!("Log: {}", value[1]);
                    println!("-----------------------");
                }
            }*/
        } else {
            eprintln!("Error: {:?}", response.status());
        }

        Ok(())
    }
}

