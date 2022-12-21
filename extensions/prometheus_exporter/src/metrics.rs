// https://github.com/prometheus/client_rust/blob/master/examples/actix-web.rs
use signal_hook::flag;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{thread, time};

use actix_web::{web, App, HttpResponse, HttpServer, Result};

use tokio::task;

use prometheus_client::encoding::text::encode;
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;

pub mod query;

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct UptimeLabels {
    pub label: String,
}

pub struct Metrics {
    uptime: Family<(), Gauge>,
}

impl Metrics {
    pub fn pg_uptime(&self, uptime: i64) {
        self.uptime.get_or_create(&()).set(uptime);
    }
}

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

pub async fn update_metrics(metrics_clone: Arc<Mutex<Metrics>>) {
    let term = Arc::new(AtomicBool::new(false));
    flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term)).unwrap();

    let metrics = metrics_clone.lock().unwrap();
    while !term.load(Ordering::Relaxed) {
        let uptime: i64 = query::handle_query(query::UPTIME_QUERY).unwrap();
        {
            metrics.pg_uptime(uptime);
            thread::sleep(time::Duration::from_millis(2500));
        }
    }
}

#[actix_web::main]
pub async fn serve() -> std::io::Result<()> {
    let metrics = Metrics {
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
        update_metrics(metrics_clone).await;
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
