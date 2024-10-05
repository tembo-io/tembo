//! Custom entrypoint for background running services

use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use gateway::config::Config;
use gateway::events_reporter::run_events_reporter;
use log::info;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let cfg = Config::new().await;

    let background_threads: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>> =
        Arc::new(Mutex::new(Vec::new()));

    let mut background_threads_guard = background_threads.lock().await;

    if cfg.run_billing_reporter {
        info!("Spawning AI billing reporter thread");

        let pg_conn = cfg.pg_conn_str.clone();
        let billing_queue_conn = cfg.billing_queue_conn_str.clone();

        background_threads_guard.push(tokio::spawn(async move {
            loop {
                if let Err(err) =
                    run_events_reporter(pg_conn.clone(), billing_queue_conn.clone()).await
                {
                    log::error!("Tembo AI billing reporter error: {err}");
                    log::info!("Restarting Tembo AI billing reporter in 30 sec");
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        }));
    }

    std::mem::drop(background_threads_guard);

    let server_port = std::env::var("PORT")
        .unwrap_or_else(|_| String::from("8080"))
        .parse::<u16>()
        .unwrap_or(8080);

    info!(
        "Starting Tembo AI Billing actix-web server on http://0.0.0.0:{}",
        server_port
    );

    let _ = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(background_threads.clone()))
            .service(web::scope("/health").service(background_threads_running))
    })
    .workers(4)
    .bind(("0.0.0.0", server_port))?
    .run()
    .await;

    Ok(())
}

#[get("/lively")]
pub async fn background_threads_running(
    background_threads: web::Data<Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>>,
) -> impl Responder {
    let background_threads = background_threads.lock().await;

    for thread in background_threads.iter() {
        if thread.is_finished() {
            return HttpResponse::InternalServerError()
                .body("One or more background tasks are not running.");
        }
    }

    HttpResponse::Ok().json("ok")
}
