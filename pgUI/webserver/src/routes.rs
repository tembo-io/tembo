use actix_web::{get, post, HttpResponse, Responder};
use regex::Regex;

#[get("/")]
pub async fn running() -> impl Responder {
    HttpResponse::Ok().body("Webserver is up and running!")
}

#[post("/connection")]
pub async fn connection(conn: String) -> impl Responder {
    // Receive postgres connection string
    // Validate connection string format

    let re = Regex::new(r"(postgres|postgresql)://[a-zA-Z][0-9a-zA-Z_-]*:[a-zA-Z][0-9a-zA-Z_-]*@[a-zA-Z][0-9a-zA-Z_-]*:[0-9]*/[a-zA-Z][0-9a-zA-Z_-]*$").unwrap();
    if !re.is_match(&conn) {
        println!("Connection string is improperly formatted");
        HttpResponse::BadRequest().body("")
    } else {
        HttpResponse::Ok().body("Connection string saved")
    }
    // Ensure connection string table exists
    // Encrypt connection string
    // Write connection info to table
}

#[post("/get-queries")]
pub async fn get_queries() -> impl Responder {
    // Receive query range (time?)
    // Connect to postgres (how will we know which instance to connect to? for now, assume there is
    //  only one connection string stored)
    // Query for range of queries
    // Return results in response
    HttpResponse::Ok().body("Queries...")
}
