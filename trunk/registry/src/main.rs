use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use trunk_registry::connect;
use trunk_registry::{config, download, publish, routes};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    // load configurations from environment
    let cfg = config::Config::default();

    let conn = connect(&cfg.database_url)
        .await
        .expect("error connecting to database");

    // run database migrations
    sqlx::migrate!()
        .run(&conn)
        .await
        .expect("error running migrations");

    HttpServer::new(move || {
        let cors = Cors::permissive();
        App::new()
            .wrap(cors)
            .app_data(web::Data::new(conn.clone()))
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
