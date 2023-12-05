use actix_web::{get, HttpResponse, Responder};

#[get("")]
pub async fn ok() -> impl Responder {
    HttpResponse::Ok().json("ok")
}
