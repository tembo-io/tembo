use actix_web::{web, App, HttpServer};
use pgui_api::routes::get_queries;
use pgui_api::{config, connect, routes};

// pgUI will make requests to this webserver in order to retrieve data it needs to present (SQL query
// data, time series data, etc)

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // load configurations from environment
    let cfg = config::Config::default();
    // Initialize connection to backend postgresql server
    let conn = connect(&cfg.pg_conn_str).await.unwrap();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(conn.clone()))
            .service(routes::running)
            .service(routes::connection)
            .service(get_queries)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
