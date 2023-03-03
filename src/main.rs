use actix_web::{App, HttpServer};
use extension_registry::routes;

// description

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
            .service(routes::running)
            .service(routes::get_extensions)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

// get all
// respond with list of all extensions in some bucket (names, relevant metadata)
