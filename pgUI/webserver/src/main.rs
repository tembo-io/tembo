use actix_web::{App, HttpServer};
use webserver::routes;
use webserver::routes::get_queries;

// UI will make requests to this webserver in order to retrieve data it needs to present (SQL query
// data, time series data, etc)

// Webserver should take the request, run some SQL query and return the results

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(routes::running)
            .service(routes::connection)
            .service(get_queries)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
