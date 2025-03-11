use crate::config::Config;
use crate::metrics::types::{InstantQuery, RangeQuery};
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

    match status_code.as_u16() {
        200 => HttpResponse::Ok().json(json_response),
        400 => HttpResponse::BadRequest().json("Prometheus reported the query is malformed"),
        504 | 503 => HttpResponse::GatewayTimeout().json("Prometheus timeout"),
        422 => {
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
            HttpResponse::InternalServerError().json(format!(
                "Unexpected response from Prometheus: {}",
                status_code
            ))
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

    // Parse step into seconds
    let step_seconds = match parse_duration(&step) {
        Ok(duration) => duration.as_secs(),
        Err(_) => return HttpResponse::BadRequest().json("Invalid step format"),
    };

    // Check if the time range and step will result in too many samples
    let start_sec = start.parse::<u64>().unwrap();
    let end_sec = end.parse::<u64>().unwrap();
    let time_range_seconds = end_sec - start_sec;
    let expected_samples = time_range_seconds / step_seconds;

    if expected_samples > 10_000 && !query.starts_with("ALERTS{") {
        return HttpResponse::BadRequest()
            .json("Query would result in too many samples. Please adjust time range or step to sample less than 10,000 time periods.");
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

fn parse_duration(duration: &str) -> Result<Duration, &'static str> {
    if duration.is_empty() {
        return Err("Duration cannot be empty");
    }

    let mut total_seconds = 0u64;
    let mut current_number = String::new();

    for c in duration.chars() {
        if c.is_ascii_digit() {
            current_number.push(c);
        } else {
            let number = current_number
                .parse::<u64>()
                .map_err(|_| "Invalid number")?;
            current_number.clear();

            match c {
                's' => total_seconds += number,
                'm' => total_seconds += number * 60,
                'h' => total_seconds += number * 3600,
                'd' => total_seconds += number * 86400,
                'w' => total_seconds += number * 604800,
                'y' => total_seconds += number * 31536000,
                _ => return Err("Invalid duration unit"),
            }
        }
    }

    if !current_number.is_empty() {
        return Err("Invalid duration format");
    }

    Ok(Duration::from_secs(total_seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_valid_inputs() {
        assert_eq!(parse_duration("5s").unwrap(), Duration::from_secs(5));
        assert_eq!(parse_duration("2m").unwrap(), Duration::from_secs(120));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("1d").unwrap(), Duration::from_secs(86400));
        assert_eq!(parse_duration("1w").unwrap(), Duration::from_secs(604800));
        assert_eq!(parse_duration("1y").unwrap(), Duration::from_secs(31536000));
        assert_eq!(parse_duration("1h30m").unwrap(), Duration::from_secs(5400));
        assert_eq!(
            parse_duration("2d12h").unwrap(),
            Duration::from_secs(216000)
        );
    }

    #[test]
    fn test_parse_duration_invalid_inputs() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("5").is_err());
        assert!(parse_duration("m5").is_err());
        assert!(parse_duration("5x").is_err());
        assert!(parse_duration("5m6").is_err());
        assert!(parse_duration("5.5h").is_err());
    }
}
