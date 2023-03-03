use actix_web::{web, App, HttpServer};
use extension_registry::{config, routes};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // load configurations from environment
    let cfg = config::S3Config::default();
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(cfg.clone()))
            .service(routes::running)
            .service(routes::get_extensions)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
