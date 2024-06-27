use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};
use std::time::Duration;

#[actix_web::main]
async fn main() {
    env_logger::init();
    let cfg = gateway::config::Config::new().await;
    let startup_configs = gateway::server::webserver_startup_config(cfg).await;
    let server_port = startup_configs.cfg.server_port;
    let server_workers = startup_configs.cfg.server_workers;
    let _ = HttpServer::new(move || {
        let cors = Cors::permissive();

        App::new()
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(startup_configs.cfg.clone()))
            .app_data(web::Data::new(startup_configs.http_client.clone()))
            .app_data(web::Data::new(startup_configs.pool.clone()))
            .app_data(web::Data::new(startup_configs.validation_cache.clone()))
            .configure(gateway::server::webserver_routes)
    })
    .workers(server_workers as usize)
    .keep_alive(Duration::from_secs(75))
    .bind(("0.0.0.0", server_port))
    .unwrap()
    .run()
    .await;
}
