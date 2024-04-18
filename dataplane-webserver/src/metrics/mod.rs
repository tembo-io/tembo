use crate::config::Config;
use crate::metrics::types::{InstantQuery, RangeQuery};
use actix_web::http::StatusCode;
use actix_web::web::{Data, Query};
use actix_web::HttpResponse;
use log::error;
use reqwest::{Client, Response};
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
pub mod expression_validator;
pub mod types;

async fn prometheus_response(response: Response) -> HttpResponse {
    let status_code = response.status();
    let json_response: Value = match response.json().await {
        Ok(response) => response,
        Err(e) => {
            error!("Failed to parse Prometheus response: {}", e);
            return HttpResponse::InternalServerError().json("Failed to parse Prometheus response");
        }
    };

    match status_code {
        StatusCode::OK => HttpResponse::Ok().json(json_response),
        StatusCode::BAD_REQUEST => {
            HttpResponse::BadRequest().json("Prometheus reported the query is malformed")
        }
        StatusCode::GATEWAY_TIMEOUT | StatusCode::SERVICE_UNAVAILABLE => {
            HttpResponse::GatewayTimeout().json("Prometheus timeout")
        }
        StatusCode::UNPROCESSABLE_ENTITY => {
            if json_response["error"]
                .to_string()
                .contains("context deadline exceeded")
            {
                HttpResponse::GatewayTimeout().json("Prometheus timeout")
            } else {
                HttpResponse::BadRequest().json("Expression cannot be executed on Prometheus")
            }
        }
        _ => {
            error!("{:?}: {:?}", status_code, &json_response);
            HttpResponse::InternalServerError()
                .json("Prometheus returned an unexpected status code")
        }
    }
}

pub async fn query_prometheus_instant(
    cfg: Data<Config>,
    http_client: Data<Client>,
    instant_query: Query<InstantQuery>,
    namespace: String,
) -> HttpResponse {
    let query =
        match expression_validator::check_query_only_accesses_namespace(&instant_query, &namespace)
        {
            Ok(value) => value,
            Err(http_response) => return http_response,
        };

    let time = instant_query.time.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs()
    });

    let timeout = format!("{}ms", cfg.prometheus_timeout_ms);
    let query_url = format!("{}/api/v1/query", cfg.prometheus_url.trim_end_matches('/'));
    let query_params = [
        ("query", &query),
        ("time", &time.to_string()),
        ("timeout", &timeout),
    ];

    let response = http_client
        .get(&query_url)
        .query(&query_params)
        .timeout(Duration::from_millis(
            cfg.prometheus_timeout_ms as u64 + 500,
        ))
        .send()
        .await;

    match response {
        Ok(response) => prometheus_response(response).await,
        Err(e) => {
            error!("Failed to query Prometheus: {}", e);
            HttpResponse::GatewayTimeout().json("Failed to query Prometheus")
        }
    }
}

pub async fn query_prometheus(
    cfg: Data<Config>,
    http_client: Data<Client>,
    range_query: Query<RangeQuery>,
    namespace: String,
) -> HttpResponse {
    let query =
        match expression_validator::check_query_only_accesses_namespace(&range_query, &namespace) {
            Ok(value) => value,
            Err(http_response) => return http_response,
        };

    let start = range_query.start.to_string();
    let end = range_query
        .end
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs() as f64
        })
        .to_string();

    if end.parse::<u64>().unwrap() < start.parse::<u64>().unwrap() {
        return HttpResponse::BadRequest()
            .json("End time must be greater than or equal to start time");
    }

    // Prepare step and timeout
    let step = range_query
        .step
        .clone()
        .unwrap_or_else(|| "60s".to_string());
    let timeout_ms = cfg.prometheus_timeout_ms;
    let reqwest_timeout = Duration::from_millis(timeout_ms as u64 + 500);

    // Check if the time range is within the allowed limits (e.g., 1 day)
    let start_sec = start.parse::<u64>().unwrap();
    let end_sec = end.parse::<u64>().unwrap();
    if end_sec - start_sec > 86_400 && !query.starts_with("ALERTS{") {
        // 1 day in seconds
        return HttpResponse::BadRequest()
            .json("Time range too large, must be less than or equal to 1 day");
    }

    if query.starts_with("ALERTS{") && end_sec - start_sec > 2_678_400 {
        // 31 days in seconds
        return HttpResponse::BadRequest()
            .json("Time range too large, must be less than or equal to 31 days for ALERT metrics");
    }

    // Construct query URL
    let query_url = format!(
        "{}/api/v1/query_range",
        cfg.prometheus_url.trim_end_matches('/')
    );
    let query_params = [
        ("query", &query),
        ("start", &start),
        ("end", &end),
        ("step", &step),
        ("timeout", &timeout_ms.to_string()),
    ];

    // Create an HTTP request to the Prometheus backend
    let response = http_client
        .get(&query_url)
        .query(&query_params)
        .timeout(reqwest_timeout)
        .send()
        .await;

    // Handle the response
    match response {
        Ok(response) => prometheus_response(response).await,
        Err(e) => {
            error!("Failed to query Prometheus: {}", e);
            HttpResponse::GatewayTimeout().json("Failed to query Prometheus")
        }
    }
}
