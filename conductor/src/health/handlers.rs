use actix_web::{get, web, HttpResponse, Responder};
use log::{debug, error};
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;

use super::AppState;

#[get("/lively")]
pub async fn background_threads_running(state: web::Data<AppState>) -> impl Responder {
    debug!("Processing health check request");

    // RwLock::read() returns the guard directly
    let health = state.task_health.read().await;
    let now = Instant::now();

    let task_status: Vec<_> = health
        .values()
        .filter_map(|task| {
            let time_since_heartbeat = now.duration_since(task.last_heartbeat);

            if time_since_heartbeat > state.config.health_timeout
                || !task.is_healthy
                || task.consecutive_errors >= state.config.max_errors
            {
                Some(json!({
                    "task_name": task.name,
                    "issues": {
                        "timeout_exceeded": time_since_heartbeat > state.config.health_timeout,
                        "unhealthy_state": !task.is_healthy,
                        "error_count": task.consecutive_errors,
                        "seconds_since_heartbeat": time_since_heartbeat.as_secs()
                    }
                }))
            } else {
                None
            }
        })
        .collect();

    if task_status.is_empty() {
        debug!("All tasks healthy");
        HttpResponse::Ok().json(json!({
            "status": "ok",
            "message": "All background tasks are healthy",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "task_count": health.len()
        }))
    } else {
        error!("Unhealthy tasks detected: {:?}", task_status);
        HttpResponse::ServiceUnavailable().json(json!({
            "status": "error",
            "message": "One or more background tasks are unhealthy",
            "unhealthy_tasks": task_status,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "total_tasks": health.len()
        }))
    }
}

#[get("/status")]
pub async fn detailed_health_check(state: web::Data<AppState>) -> impl Responder {
    debug!("Processing detailed health check request");

    let health = state.task_health.read().await;
    let now = Instant::now();

    let task_statuses: HashMap<String, serde_json::Value> = health
        .iter()
        .map(|(name, task)| {
            let time_since_heartbeat = now.duration_since(task.last_heartbeat);
            (
                name.clone(),
                json!({
                    "name": task.name,
                    "healthy": task.is_healthy,
                    "consecutive_errors": task.consecutive_errors,
                    "last_heartbeat": {
                        "seconds_ago": time_since_heartbeat.as_secs(),
                        "exceeds_timeout": time_since_heartbeat > state.config.health_timeout
                    },
                    "status": if task.is_healthy
                        && task.consecutive_errors == 0
                        && time_since_heartbeat <= state.config.health_timeout
                        { "healthy" } else { "unhealthy" },
                    "metrics": {
                        "error_rate": task.consecutive_errors as f64 /
                            (time_since_heartbeat.as_secs() as f64 + 1.0)
                    }
                }),
            )
        })
        .collect();

    HttpResponse::Ok().json(json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "config": {
            "max_errors": state.config.max_errors,
            "health_timeout_secs": state.config.health_timeout.as_secs()
        },
        "tasks": task_statuses,
        "summary": {
            "total_tasks": health.len(),
            "healthy_tasks": task_statuses.values()
                .filter(|v| v.get("status").and_then(|s| s.as_str()) == Some("healthy"))
                .count(),
        }
    }))
}
