use actix_web::{get, post, web, HttpResponse, Responder};
use base64::{engine::general_purpose, Engine as _};
use regex::Regex;
use sqlx::{Pool, Postgres};

#[get("/")]
pub async fn running() -> impl Responder {
    HttpResponse::Ok().body("Webserver is up and running!")
}

#[post("/connection")]
pub async fn connection(conn_str: String, conn: web::Data<Pool<Postgres>>) -> impl Responder {
    // Receive postgres connection string
    // Validate connection string format
    // TODO(ianstanton) regex needs to be tweaked a bit
    let re = Regex::new(r"(postgres|postgresql)://[a-zA-Z][0-9a-zA-Z_-]*:[a-zA-Z][0-9a-zA-Z_-]*@[a-zA-Z][0-9a-zA-Z_-]*:[0-9]*/[a-zA-Z][0-9a-zA-Z_-]*$").unwrap();
    if !re.is_match(&conn_str) {
        println!("Connection string is improperly formatted");
        HttpResponse::BadRequest().body("")
    } else {
        // Connect to postgres
        let mut tx = conn.begin().await.unwrap();
        // Ensure connection string table exists
        sqlx::query("CREATE TABLE IF NOT EXISTS conn_str (conn text);")
            .execute(&mut tx)
            .await
            .unwrap();
        // Encrypt connection string
        let conn_b64 = general_purpose::STANDARD.encode(conn_str);
        println!("{}", conn_b64);
        // Write connection info to table
        sqlx::query(format!("INSERT INTO conn_str VALUES ('{}');", conn_b64).as_str())
            .execute(&mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        HttpResponse::Ok().body("Connection string saved")
    }
    // Encrypt connection string
    // Write connection info to table
}

#[post("/get-queries")]
pub async fn get_queries(conn: web::Data<Pool<Postgres>>) -> impl Responder {
    // Receive query range (time?)
    // Connect to postgres (how will we know which instance to connect to? for now, assume there is
    //  only one connection string stored)
    let mut tx = conn.begin().await.unwrap();
    sqlx::query("CREATE TABLE IF NOT EXISTS ")
        .execute(&mut tx)
        .await
        .unwrap();

    tx.commit().await.unwrap();

    // Query for range of queries
    // Return results in response
    HttpResponse::Ok().body("Queries...")
}
