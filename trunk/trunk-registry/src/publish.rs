//! Functionality related to publishing a new extension or version of an extension.

use crate::config::Config;
use crate::errors::ExtensionRegistryError;
use crate::errors::ExtensionRegistryError::AuthorizationError;
use crate::uploader::upload_extension;
use crate::views::extension_publish::ExtensionUpload;
use actix_multipart::Multipart;
use actix_web::http::header::AUTHORIZATION;
use actix_web::{error, post, web, HttpResponse};
use aws_config::SdkConfig;
use aws_sdk_s3;
use aws_sdk_s3::primitives::ByteStream;
use futures::TryStreamExt;
use sqlx::{Pool, Postgres};

const MAX_SIZE: usize = 262_144; // max payload size is 256k

/// Handles the `POST /extensions/new` route.
/// Used by `trunk publish` to publish a new extension or to publish a new version of an
/// existing extension.

#[post("/extensions/new")]
pub async fn publish(
    cfg: web::Data<Config>,
    conn: web::Data<Pool<Postgres>>,
    aws_config: web::Data<SdkConfig>,
    mut payload: Multipart,
) -> Result<HttpResponse, ExtensionRegistryError> {
    // Get request body
    let mut metadata = web::BytesMut::new();
    let mut file = web::BytesMut::new();
    while let Some(mut field) = payload.try_next().await? {
        let headers = field.headers();
        let auth = headers.get(AUTHORIZATION).unwrap();
        if auth != "" {
            return Err(AuthorizationError());
        }
        // Field is stream of Bytes
        while let Some(chunk) = field.try_next().await? {
            // limit max size of in-memory payload
            if (chunk.len()) > MAX_SIZE {
                return Err(ExtensionRegistryError::from(error::ErrorBadRequest(
                    "overflow",
                )));
            }
            if field.name() == "metadata" {
                metadata.extend_from_slice(&chunk);
            } else if field.name() == "file" {
                file.extend_from_slice(&chunk);
            }
        }
    }

    // Deserialize body
    let new_extension = serde_json::from_slice::<ExtensionUpload>(&metadata)?;

    // Create a transaction on the database, if there are no errors,
    // commit the transactions to record a new or updated extension.
    let mut tx = conn.begin().await?;

    // Validate name input
    check_input(&new_extension.name)?;

    // Check if extension exists
    let exists = sqlx::query!(
        "SELECT * FROM extensions WHERE name = $1",
        new_extension.name
    )
    .fetch_optional(&mut tx)
    .await?;

    match exists {
        // TODO(ianstanton) Refactor into separate functions
        Some(exists) => {
            // Extension exists
            let mut tx = conn.begin().await?;
            let extension_id = exists.id;

            // Check if version exists
            let version_exists = sqlx::query!(
                "SELECT *
                FROM versions
                WHERE 
                    extension_id = $1
                    and num = $2",
                extension_id as i32,
                new_extension.vers.to_string()
            )
            .fetch_optional(&mut tx)
            .await?;

            match version_exists {
                Some(_version_exists) => {
                    // Update updated_at timestamp
                    sqlx::query!(
                        "UPDATE versions
                    SET updated_at = (now() at time zone 'utc'), license = $1
                    WHERE extension_id = $2
                    AND num = $3",
                        new_extension.license,
                        extension_id as i32,
                        new_extension.vers.to_string()
                    )
                    .execute(&mut tx)
                    .await?;
                }
                None => {
                    // Create new record in versions table
                    sqlx::query!(
                        "
                    INSERT INTO versions(extension_id, num, created_at, yanked, license)
                    VALUES ($1, $2, (now() at time zone 'utc'), $3, $4)
                    ",
                        extension_id as i32,
                        new_extension.vers.to_string(),
                        false,
                        new_extension.license
                    )
                    .execute(&mut tx)
                    .await?;
                }
            }

            // Set updated_at time on extension
            sqlx::query!(
                "UPDATE extensions
            SET updated_at = (now() at time zone 'utc'), description = $1, documentation = $2, homepage = $3, repository = $4
            WHERE name = $5",
                new_extension.description,
                new_extension.documentation,
                new_extension.homepage,
                new_extension.repository,
                new_extension.name,
            )
            .execute(&mut tx)
            .await?;
            tx.commit().await?;
        }
        None => {
            // Else, create new record in extensions table
            let mut tx = conn.begin().await?;
            let id_row = sqlx::query!(
                "
            INSERT INTO extensions(name, created_at, updated_at, description, homepage, documentation, repository)
            VALUES ($1, (now() at time zone 'utc'), (now() at time zone 'utc'), $2, $3, $4, $5)
            RETURNING id
            ",
                new_extension.name,
                new_extension.description,
                new_extension.homepage,
                new_extension.documentation,
                new_extension.repository
            )
            .fetch_one(&mut tx)
            .await?;
            let extension_id = id_row.id;

            // Create new record in versions table
            sqlx::query!(
                "
            INSERT INTO versions(extension_id, num, created_at, yanked, license)
            VALUES ($1, $2, (now() at time zone 'utc'), $3, $4)
            ",
                extension_id as i32,
                new_extension.vers.to_string(),
                false,
                new_extension.license
            )
            .execute(&mut tx)
            .await?;
            tx.commit().await?;
        }
    }

    // TODO(ianstanton) Generate checksum
    let file_byte_stream = ByteStream::from(file.freeze());
    let client = aws_sdk_s3::Client::new(&aws_config);
    upload_extension(
        &cfg.bucket_name,
        &client,
        file_byte_stream,
        &new_extension,
        &new_extension.vers,
    )
    .await?;
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
