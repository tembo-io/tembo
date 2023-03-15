use actix_web::{get, HttpResponse, Responder};

#[get("/")]
pub async fn running() -> impl Responder {
    HttpResponse::Ok().body("API is up and running!")
}
