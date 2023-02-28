use crate::connect;
use actix_web::{get, post, web, HttpResponse, Responder};
use base64::{engine::general_purpose, Engine as _};
use regex::Regex;
use sqlx::postgres::PgRow;
use sqlx::{Error, Pool, Postgres, Row};

#[get("/")]
pub async fn running() -> impl Responder {
    HttpResponse::Ok().body("API is up and running!")
}

#[post("/connection")]
pub async fn connection(conn_str: String, conn: web::Data<Pool<Postgres>>) -> impl Responder {
    // Validate connection string format
    // TODO(ianstanton) regex needs to be tweaked a bit (password)
    let re = Regex::new(r"(postgres|postgresql)://[a-zA-Z][0-9a-zA-Z_-]*:[a-zA-Z][0-9a-zA-Z_-]*@[a-zA-Z][0-9a-zA-Z_-]*:[0-9]*/[a-zA-Z][0-9a-zA-Z_-]*$").unwrap();
    if !re.is_match(&conn_str) {
        println!("Connection string is improperly formatted");
        HttpResponse::BadRequest().body("")
    } else {
        // Connect to postgres
        let mut tx = conn.begin().await.unwrap();
        // Ensure connection string table exists
        sqlx::query("CREATE TABLE IF NOT EXISTS conn_str (id int, conn text);")
            .execute(&mut tx)
            .await
            .unwrap();
        // base64 encode connection string
        // TODO(ianstanton) Properly encrypt connection string
        let conn_b64 = general_purpose::STANDARD.encode(conn_str);
        // Create identifier for conn string
        // Write connection info to table
        // TODO(ianstanton) Accept user provided ID and use in INSERT
        sqlx::query(format!("INSERT INTO conn_str VALUES (1, '{}');", conn_b64).as_str())
            .execute(&mut tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        HttpResponse::Ok().body("Connection string saved")
    }
}

#[get("/get-queries")]
pub async fn get_queries(conn: web::Data<Pool<Postgres>>) -> impl Responder {
    let mut queries: Vec<(f64, f64, String)> = Vec::new();
    // Connect to backend postgresql server and query for connection string
    let mut tx = conn.begin().await.unwrap();
    // TODO(ianstanton) Query conn_str for a connection string with a given ID
    let row: Result<PgRow, Error> = sqlx::query("SELECT * FROM conn_str;")
        .fetch_one(&mut tx)
        .await;
    tx.commit().await.unwrap();
    // Connect to postgres instance
    let conn_str_b64: String = row.unwrap().get(1);
    // Decode connection string
    let conn_str = b64_decode(&conn_str_b64);
    let new_conn = connect(&conn_str).await.unwrap();
    tx = new_conn.begin().await.unwrap();
    let query = "SELECT (total_exec_time / 1000 / 60) as total, (total_exec_time/calls) as avg, query FROM pg_stat_statements ORDER BY 1 DESC LIMIT 10;";
    let rows: Result<Vec<PgRow>, Error> = sqlx::query(query).fetch_all(&mut tx).await;
    for row in rows.unwrap().iter() {
        queries.push((row.get(0), row.get(1), row.get(2)));
    }
    // Return results in response
    HttpResponse::Ok().body(format!("Queries... {:?}", queries))
}

fn b64_decode(b64_encoded: &str) -> String {
    let bytes = general_purpose::STANDARD.decode(b64_encoded).unwrap();
    std::str::from_utf8(&bytes).unwrap().to_owned()
}
