use actix_web::{web, App, HttpServer};
use trunk_registry::{config, download, publish, routes};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    // load configurations from environment
    let cfg = config::Config::default();
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(cfg.clone()))
            .service(routes::running)
            .service(routes::get_all_extensions)
            .service(publish::publish)
            .service(download::download)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
