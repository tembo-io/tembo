use crate::download::latest_version;
use crate::errors::ExtensionRegistryError;
use actix_web::{get, web, HttpResponse, Responder};
use serde_json::{json, Value};
use sqlx::{Pool, Postgres};

#[get("/")]
pub async fn running() -> impl Responder {
    HttpResponse::Ok().body("API is up and running!")
}

#[get("/extensions/all")]
pub async fn get_all_extensions(
    conn: web::Data<Pool<Postgres>>,
) -> Result<HttpResponse, ExtensionRegistryError> {
    let mut extensions: Vec<Value> = Vec::new();

    // Create a transaction on the database, if there are no errors,
    // commit the transactions to record a new or updated extension.
    let mut tx = conn.begin().await?;
    let rows = sqlx::query!("SELECT * FROM extensions")
        .fetch_all(&mut tx)
        .await?;
    for row in rows.iter() {
        let name = row.name.to_owned().unwrap();
        let latest = latest_version(&name, conn.clone()).await?;
        let data = json!(
        {
          "name": row.name.to_owned(),
          "latestVersion": latest,
          "createdAt": row.created_at.to_string(),
          "updatedAt": row.updated_at.to_string(),
          "description": row.description.to_owned(),
          "homepage": row.homepage.to_owned(),
          "documentation": row.documentation.to_owned(),
          "repository": row.repository.to_owned()
        });
        extensions.push(data);
    }
    // Return results in response
    let json = serde_json::to_string_pretty(&extensions)?;
    Ok(HttpResponse::Ok().body(json))
}
