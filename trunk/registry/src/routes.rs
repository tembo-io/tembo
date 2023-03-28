use crate::config::Config;
use crate::connect;
use crate::errors::ExtensionRegistryError;
use actix_web::{get, web, HttpResponse, Responder};
use sqlx::Row;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Extension {
    name: String,
    description: String,
    homepage: String,
    documentation: String,
    repository: String
}

#[get("/")]
pub async fn running() -> impl Responder {
    HttpResponse::Ok().body("API is up and running!")
}

#[get("/extensions/all")]
pub async fn get_all_extensions(
    cfg: web::Data<Config>,
) -> Result<HttpResponse, ExtensionRegistryError> {
    let mut extensions: Vec<Extension> = Vec::new();
    // Set database conn
    let conn = connect(&cfg.database_url).await?;
    // Create a transaction on the database, if there are no errors,
    // commit the transactions to record a new or updated extension.
    let mut tx = conn.begin().await?;
    let query = format!("SELECT * FROM extensions",);
    let rows = sqlx::query(&query).fetch_all(&mut tx).await?;
    for row in rows.iter() {
        let ext = Extension {
            name: row.get(1),
            description: row.get(5),
            homepage: row.get(6),
            documentation: "".to_string(),
            repository: "".to_string()
        };
        extensions.push(ext);
    }
    // Return results in response
    let json = serde_json::to_string(&extensions);
    Ok(HttpResponse::Ok().body(format!("{:?}", json)))
}
