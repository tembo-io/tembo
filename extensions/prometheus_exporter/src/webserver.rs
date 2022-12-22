use actix_web::{web, App, HttpResponse, HttpServer, Result};
use std::sync::{Arc, Mutex};

use prometheus_client::encoding::text::encode;
use prometheus_client::registry::Registry;

use tokio::task;

use prometheus_client::metrics::family::Family;

use crate::metrics;

pub struct AppState {
    pub registry: Registry,
}

pub async fn metrics_handler(state: web::Data<Mutex<AppState>>) -> Result<HttpResponse> {
    let state = state.lock().unwrap();
    let mut body = String::new();
    encode(&mut body, &state.registry).unwrap();
    Ok(HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(body))
}

#[actix_web::main]
pub async fn run() -> std::io::Result<()> {
    let metrics = metrics::Metrics {
        uptime: Family::default(),
    };

    let mut state = AppState {
        registry: Registry::default(),
    };

    state.registry.register(
        "pg_uptime",
        "Postgres server uptime",
        metrics.uptime.clone(),
    );
    let state = web::Data::new(Mutex::new(state));

    let mutex_metrics = Arc::new(Mutex::new(metrics));
    let metrics_clone = Arc::clone(&mutex_metrics);

    task::spawn(async {
        metrics::update_metrics(metrics_clone).await;
    });

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(web::resource("/metrics").route(web::get().to(metrics_handler)))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
