use actix_web::{get, HttpResponse, Responder};

#[get("/ready")]
pub async fn ready() -> impl Responder {
    HttpResponse::Ok().json("ready")
}

#[get("/lively")]
pub async fn lively() -> impl Responder {
    HttpResponse::Ok().json("alive")
}
