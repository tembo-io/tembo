use actix_web::{web, App, HttpServer};
use trunk_registry::{config, publish, routes};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // load configurations from environment
    let cfg = config::Config::default();
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(cfg.clone()))
            .service(routes::running)
            .service(routes::get_extensions)
            .service(publish::publish)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
