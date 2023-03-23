use crate::config::Config;
use crate::connect;
use crate::errors::ExtensionRegistryError;
use actix_web::{get, web, HttpResponse, Responder};
use sqlx::{Connection, Row};

#[get("/")]
pub async fn running() -> impl Responder {
    HttpResponse::Ok().body("API is up and running!")
}

#[get("/extensions/all")]
pub async fn get_all_extensions(
    cfg: web::Data<Config>,
) -> Result<HttpResponse, ExtensionRegistryError> {
    let mut extensions: Vec<String> = Vec::new();
    // Set database conn
    let conn = connect(&cfg.database_url).await?;
    // Create a transaction on the database, if there are no errors,
    // commit the transactions to record a new or updated extension.
    let mut tx = conn.begin().await?;
    let query = format!("SELECT * FROM extensions",);
    let rows = sqlx::query(&query).fetch_all(&mut tx).await?;
    for row in rows.iter() {
        extensions.push(row.get(1));
    }
    // Return results in response
    Ok(HttpResponse::Ok().body(format!("Extensions... {:?}", extensions)))
}
