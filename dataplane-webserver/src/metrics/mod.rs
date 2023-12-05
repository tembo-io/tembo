use crate::config::Config;
use crate::metrics::types::RangeQuery;
use actix_web::http::StatusCode;
use actix_web::web::{Data, Query};
use actix_web::HttpResponse;
use log::{debug, error, warn};
use reqwest::{Client, Url};
use serde_json::Value;
use std::time::{Duration, SystemTime};

pub mod expression_validator;
pub mod types;

pub async fn query_prometheus(
    cfg: Data<Config>,
    http_client: Data<Client>,
    range_query: Query<RangeQuery>,
    namespace: String,
) -> HttpResponse {
    let query = match expression_validator::check_query_only_accesses_namespace(
        &range_query.clone(),
        &namespace,
    ) {
        Ok(value) => value,
        Err(http_response) => return http_response,
    };

    let mut start = range_query.start;
    // If 'end' query parameter was provided, use it. Otherwise use current time.
    let end = match range_query.end {
        Some(end) => end,
        None => match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => n.as_secs_f64(),
            Err(_) => {
                error!("Failed to get current time");
                return HttpResponse::InternalServerError().json("Failed to get current time");
            }
        },
    };
    let step = match range_query.step.clone() {
        Some(step) => step,
        None => "60s".to_string(),
    };
    // Check that end - start is not greater than 1 day, plus 100 seconds
    if end - start > 86500.0 {
        warn!(
            "Query time range too large: namespace '{}', start '{}', end '{}'",
            namespace, start, end
        );
        return HttpResponse::BadRequest()
            .json("Query time range too large, must be less than or equal to 1 day");
    }
    if end < start {
        start = end.clone();
    }

    // Get timeout from config
    let prometheus_timeout_ms = cfg.prometheus_timeout_ms;
    // Set reqwest timeout to 50% greater than the prometheus timeout, plus 500ms, since we
    // prefer for Prometheus to perform the timeout rather than reqwest client.
    let reqwest_timeout_ms = prometheus_timeout_ms + (prometheus_timeout_ms / 2) + 500;
    let reqwest_timeout_ms: u64 = match reqwest_timeout_ms.try_into() {
        Ok(n) => n,
        Err(_) => {
            error!("Failed to convert timeout to u64");
            return HttpResponse::InternalServerError().json("Failed to convert timeout");
        }
    };
    let timeout = format!("{prometheus_timeout_ms}ms");

    let query_params = vec![
        ("query", query),
        ("start", start.to_string()),
        ("end", end.to_string()),
        ("step", step),
        ("timeout", timeout),
    ];
    // Get prometheus URL from config
    let prometheus_url = cfg.prometheus_url.clone();
    // trim trailing slash
    let prometheus_url = prometheus_url.trim_end_matches('/');
    let prometheus_url = format!("{}/api/v1/query_range", prometheus_url);

    let query_url = match Url::parse_with_params(&prometheus_url, &query_params) {
        Ok(url) => url,
        Err(e) => {
            error!("Failed to parse Prometheus URL: {}", e);
            return HttpResponse::InternalServerError()
                .json("Failed to create URL to query Prometheus");
        }
    };
    debug!("{}", query_url);

    // Create an HTTP request to the Prometheus backend
    let prometheus_response = match http_client
        .get(query_url)
        .timeout(Duration::from_millis(reqwest_timeout_ms))
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => {
            error!("Failed to query Prometheus: {}", e);
            return HttpResponse::GatewayTimeout().json("Failed to query Prometheus");
        }
    };
    debug!("{:?}", &prometheus_response);

    let status_code = prometheus_response.status();

    let json_response = match prometheus_response.json::<Value>().await {
        Ok(response) => response,
        Err(e) => {
            error!("Failed to parse Prometheus response: {}", e);
            return HttpResponse::InternalServerError()
                .json("Failed to parse Prometheus response in JSON");
        }
    };

    match status_code {
        StatusCode::OK => {
            debug!("Request to prometheus returned 200");
        }
        StatusCode::BAD_REQUEST => {
            warn!("{:?}", &json_response);
            return HttpResponse::BadRequest().json("Prometheus reported the query is malformed");
        }
        StatusCode::GATEWAY_TIMEOUT | StatusCode::SERVICE_UNAVAILABLE => {
            warn!("{:?}", &json_response);
            return HttpResponse::GatewayTimeout().json("Prometheus timeout");
        }
        StatusCode::UNPROCESSABLE_ENTITY => {
            // If this is a timeout, then make the response 503 to match the other
            // types of timeouts.
            warn!("{:?}", &json_response);
            if json_response["error"]
                .to_string()
                .contains("context deadline exceeded")
            {
                return HttpResponse::GatewayTimeout().json("Prometheus timeout");
            }
            return HttpResponse::BadRequest().json("Expression cannot be executed on Prometheus");
        }
        _ => {
            error!("{:?}: {:?}", status_code, &json_response);
            return HttpResponse::InternalServerError()
                .json("Prometheus returned an unexpected status code");
        }
    }
    // return json response from prometheus to client
    HttpResponse::Ok().json(json_response)
}
