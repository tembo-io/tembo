use crate::items::{Items};
use crate::error_handler::CustomError;
use actix_web::{get, web, HttpResponse};

#[get("/read")]
async fn find_all() -> Result<HttpResponse, CustomError> {
    let items = web::block(|| Items::find_all()).await.unwrap();
    Ok(HttpResponse::Ok().json(items))
}


pub fn init_routes(config: &mut web::ServiceConfig) {
    config.service(find_all);
}
