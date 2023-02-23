use actix_web::{get, post, App, HttpResponse, HttpServer, Responder};

// UI will make requests to this webserver in order to retrieve data it needs to present (SQL query
// data, time series data, etc)

// Webserver should take the request, run some SQL query and return the results

#[get("/")]
async fn running() -> impl Responder {
    HttpResponse::Ok().body("Webserver is up and running!")
}

#[post("/connection")]
async fn connection(req_body: String) -> impl Responder {
    // Receive postgres connection string
    // Validate connection string format
    // Ensure connection string table exists
    // Encrypt connection string
    // Write connection info to table
    HttpResponse::Ok().body("Connection string saved")
}

#[post("/get-queries")]
async fn get_queries() -> impl Responder {
    // Receive query range (time?)
    // Connect to postgres (how will we know which instance to connect to? for now, assume there is
    //  only one connection string stored)
    // Query for range of queries
    // Return results in response
    HttpResponse::Ok().body("Queries...")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(running)
            .service(connection)
    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
