//! Functionality related to publishing a new extension or version of an extension.

use crate::config::Config;
use crate::connect;
use crate::errors::ExtensionRegistryError;
use crate::views::extension_publish::ExtensionUpload;
use actix_web::{error, post, web, HttpResponse, Responder};
use futures::StreamExt;
use sqlx::Row;

const MAX_SIZE: usize = 262_144; // max payload size is 256k

/// Handles the `POST /extensions/new` route.
/// Used by `trunk publish` to publish a new extension or to publish a new version of an
/// existing extension.

#[post("/extensions/new")]
pub async fn publish(cfg: web::Data<Config>, mut payload: web::Payload) -> impl Responder {
    // Get request body
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        // limit max size of in-memory payload
        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(error::ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    // Deserialize body
    let new_extension = serde_json::from_slice::<ExtensionUpload>(&body)?;

    // Set database conn
    let conn = connect(&cfg.database_url)
        .await
        .expect("Error establishing connection");

    // Create a transaction on the database, if there are no errors,
    // commit the transactions to record a new or updated extension.
    let mut tx = conn.begin().await.expect("Error creating transaction");

    // Validate name input
    check_input(&new_extension.name).expect("Invalid format for name");

    // Check if extension exists
    let query = format!(
        "SELECT * FROM extensions WHERE name = '{}' IS TRUE",
        new_extension.name
    );

    let exists = sqlx::query(&query)
        .fetch_optional(&mut tx)
        .await
        .expect("error");

    match exists {
        // TODO(ianstanton) Refactor into separate functions
        Some(exists) => {
            // Extension exists
            let mut tx = conn.begin().await.expect("Error creating transaction");
            let time = chrono::offset::Utc::now().naive_utc();
            let extension_id: i64 = exists.get(0);

            // Check if version exists
            let query = format!(
                "SELECT * FROM versions WHERE extension_id = {} and num = '{}' IS TRUE",
                extension_id, new_extension.vers
            );

            let version_exists = sqlx::query(&query)
                .fetch_optional(&mut tx)
                .await
                .expect("Error executing query");

            match version_exists {
                Some(_version_exists) => {
                    // Update updated_at timestamp
                    let query = format!(
                        "UPDATE versions
            SET updated_at = '{}'
            WHERE extension_id = {}
            AND num = '{}'",
                        time, extension_id, new_extension.vers
                    );
                    sqlx::query(&query)
                        .execute(&mut tx)
                        .await
                        .expect("Error executing query");
                }
                None => {
                    // Create new record in versions table
                    let query = format!(
                        "
                    INSERT INTO versions(extension_id, num, created_at, yanked, license)
                    VALUES ('{}', '{}', '{}', '{}', '{}')
                    ",
                        extension_id,
                        new_extension.vers,
                        time,
                        "f",
                        new_extension.license.unwrap()
                    );
                    sqlx::query(&query)
                        .execute(&mut tx)
                        .await
                        .expect("Error executing query");
                }
            }

            // Set updated_at time on extension
            let query = format!(
                "UPDATE extensions
            SET updated_at = '{}'
            WHERE name = '{}'",
                time, new_extension.name,
            );
            sqlx::query(&query)
                .execute(&mut tx)
                .await
                .expect("Error executing query");
            tx.commit().await.expect("Error committing transaction");
        }
        None => {
            // Else, create new record in extensions table
            let mut tx = conn.begin().await.expect("Error creating transaction");
            let time = chrono::offset::Utc::now().naive_utc();
            let query = format!(
                "
            INSERT INTO extensions(name, created_at, description, homepage)
            VALUES ('{}', '{}', '{}', '{}')
            RETURNING id
            ",
                new_extension.name,
                time,
                new_extension.description.unwrap(),
                new_extension.homepage.unwrap()
            );
            let id_row = sqlx::query(&query)
                .fetch_one(&mut tx)
                .await
                .expect("Error fetching row");
            let extension_id: i64 = id_row.get(0);

            // Create new record in versions table
            let query = format!(
                "
                    INSERT INTO versions(extension_id, num, created_at, yanked, license)
                    VALUES ('{}', '{}', '{}', '{}', '{}')
                    ",
                extension_id,
                new_extension.vers,
                time,
                "f",
                new_extension.license.unwrap()
            );
            sqlx::query(&query)
                .execute(&mut tx)
                .await
                .expect("Error executing query");
            tx.commit().await.expect("Error committing transaction");
        }
    }

    // TODO(ianstanton) Generate checksum
    // TODO(ianstanton) Upload extension tar.gz

    Ok(HttpResponse::Ok().body(format!(
        "Successfully published extension {} version {}",
        new_extension.name, new_extension.vers
    )))
}

pub fn check_input(input: &str) -> Result<(), ExtensionRegistryError> {
    let valid = input
        .as_bytes()
        .iter()
        .all(|&c| c.is_ascii_alphanumeric() || c == b'_');
    match valid {
        true => Ok(()),
        false => Err(ExtensionRegistryError::ResponseError()),
    }
}
