use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};
use std::time::Duration;

use gateway::{config, db, routes};

use sqlx::{Pool, Postgres};

#[actix_web::main]
async fn main() {
    env_logger::init();

    let cfg = config::Config::new().await;
    let server_port = cfg.server_port;
    let dbclient: Pool<Postgres> = db::connect(&cfg.pg_conn_str, 4)
        .await
        .expect("Failed to connect to database");
    sqlx::migrate!("./migrations")
        .run(&dbclient)
        .await
        .expect("Failed to run migrations");

    let reqwest_client: reqwest::Client = reqwest::Client::new();
    let _ = HttpServer::new(move || {
        let cors = Cors::permissive();

        App::new()
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(cfg.clone()))
            .app_data(web::Data::new(reqwest_client.clone()))
            .app_data(web::Data::new(dbclient.clone()))
            .default_service(web::to(routes::forward::forward_request))
    })
    .workers(8)
    .keep_alive(Duration::from_secs(75))
    .bind(("0.0.0.0", server_port))
    .unwrap()
    .run()
    .await;
}
