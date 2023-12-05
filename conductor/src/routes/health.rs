use actix_web::{get, web, HttpResponse, Responder};
use std::sync::{Arc, Mutex};

#[get("/lively")]
pub async fn background_threads_running(
    background_threads: web::Data<Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>>,
) -> impl Responder {
    let background_threads = match background_threads.lock() {
        Ok(threads) => threads,
        Err(_) => {
            return HttpResponse::InternalServerError()
                .body("Failed to check if background tasks are running.")
        }
    };
    for thread in background_threads.iter() {
        if thread.is_finished() {
            return HttpResponse::InternalServerError()
                .body("One or more background tasks are not running.");
        }
    }
    HttpResponse::Ok().json("ok")
}
