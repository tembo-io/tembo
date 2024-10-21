use actix_web::{get, web, HttpResponse, Responder};
use std::{
    ops::Not,
    sync::{Arc, Mutex},
};

use crate::heartbeat_monitor::HeartbeatMonitor;

#[get("/lively")]
pub async fn background_threads_running(
    background_threads: web::Data<Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>>,
    heartbeat_monitor: web::Data<HeartbeatMonitor>,
) -> impl Responder {
    let background_threads = match background_threads.lock() {
        Ok(threads) => threads,
        Err(_) => {
            return HttpResponse::InternalServerError()
                .body("Failed to check if background tasks are running.")
        }
    };

    if heartbeat_monitor.is_heartbeat_active().not() {
        return HttpResponse::InternalServerError()
            .body("One or more background tasks are not responding.");
    }

    for thread in background_threads.iter() {
        if thread.is_finished() {
            return HttpResponse::InternalServerError()
                .body("One or more background tasks are not running.");
        }
    }
    HttpResponse::Ok().json("ok")
}
