use actix_web::{get, HttpResponse, Responder};

#[get("/ready")]
async fn ready() -> impl Responder {
    HttpResponse::Ok().json("ready")
}

#[get("/lively")]
async fn lively() -> impl Responder {
    HttpResponse::Ok().json("alive")
}
