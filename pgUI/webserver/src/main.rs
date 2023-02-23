use actix_web::{web, App, HttpServer};
use webserver::routes::get_queries;
use webserver::{config, connect, routes};

// UI will make requests to this webserver in order to retrieve data it needs to present (SQL query
// data, time series data, etc)

// Webserver should take the request, run some SQL query and return the results

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
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
