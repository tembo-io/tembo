use crate::backups::job::get_job_status;
use crate::backups::types::JobStatus;
use crate::{
    backups::types::{BackupResult, BackupStatus},
    config,
};
use actix_web::{
    error::{ErrorInternalServerError, ErrorNotFound},
    Error,
};
use aws_sdk_s3::{
    presigning::{PresignedRequest, PresigningConfig},
    Client as S3Client,
};
use aws_smithy_types::byte_stream::ByteStream;
use serde_json::json;

/// Retrieves and parses backup metadata from an S3 object.
///
/// This function attempts to fetch a JSON metadata file from S3 and parse its contents.
/// It handles various S3 error cases and provides appropriate HTTP status codes.
///
/// # Arguments
/// * `client` - AWS S3 client for making requests
/// * `bucket` - Name of the S3 bucket containing the metadata
/// * `key` - Full path to the metadata file within the bucket
///
/// # Returns
/// * `Ok(Value)` - Parsed JSON metadata as a serde_json Value
/// * `Err(Error)` - If the metadata:
///   - Does not exist in S3 (404 Not Found)
///   - Cannot be read from S3 (500 Internal Server Error)
///   - Is not valid JSON (500 Internal Server Error)
///
/// # Error Status Codes
/// * `404 Not Found` - When the metadata file doesn't exist
/// * `500 Internal Server Error` - For S3 access errors or JSON parsing failures
pub async fn get_backup_metadata(
    client: &S3Client,
    bucket: &str,
    key: &str,
) -> Result<serde_json::Value, Error> {
    let resp = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| {
            if let Some(e) = e.as_service_error() {
                if e.is_no_such_key() {
                    return ErrorNotFound(format!("Backup metadata not found: {}", key));
                }
            }
            ErrorInternalServerError(format!("Failed to get backup metadata from S3: {}", e))
        })?;

    let body = resp.body.collect().await.map_err(|e| {
        ErrorInternalServerError(format!("Failed to read backup metadata from S3: {}", e))
    })?;

    serde_json::from_slice(&body.into_bytes())
        .map_err(|e| ErrorInternalServerError(format!("Failed to parse backup metadata: {}", e)))
}

/// Analyzes backup metadata and determines the current backup status.
///
/// This function examines the status field in the metadata JSON and constructs
/// an appropriate BackupStatus enum variant. For completed backups, it also
/// generates a pre-signed download URL.
///
/// # Arguments
/// * `metadata` - JSON metadata from S3 containing backup status information
/// * `job_id` - Unique identifier for the backup job
/// * `client` - AWS S3 client for generating pre-signed URLs
/// * `config` - Application configuration containing timeout settings
///
/// # Returns
/// * `Ok(BackupStatus)` - Current status of the backup:
///   - `Completed` with download URL and expiry for successful backups
///   - `Processing` for in-progress backups
///   - `Failed` with error message for failed backups
///   - `Unknown` for unrecognized status values
/// * `Err(Error)` - If required metadata fields are missing or URL generation fails
///
/// # Example BackupStatus JSON Responses
/// For completed backups:
/// ```json
/// {
///     "status": "completed",
///     "job_id": "550e8400-e29b-41d4-a716-446655440000",
///     "download_url": "https://my-bucket.s3.region.amazonaws.com/path/to/backup.tar.gz?signed-params",
///     "expires_at": "2024-03-21T16:30:00Z"
/// }
/// ```
///
/// For in-progress backups:
/// ```json
/// {
///     "status": "processing",
///     "job_id": "550e8400-e29b-41d4-a716-446655440000"
/// }
/// ```
///
/// For failed backups:
/// ```json
/// {
///     "status": "failed",
///     "job_id": "550e8400-e29b-41d4-a716-446655440000",
///     "error": "Failed to create backup: insufficient disk space"
/// }
/// ```
///
/// For unknown status:
/// ```json
/// {
///     "status": "unknown",
///     "job_id": "550e8400-e29b-41d4-a716-446655440000"
/// }
/// ```
pub async fn determine_backup_status(
    metadata: serde_json::Value,
    job_id: String,
    client: &S3Client,
    config: &config::Config,
) -> Result<BackupStatus, Error> {
    match metadata["status"].as_str() {
        Some("completed") => {
            let backup_key = metadata["backup_key"].as_str().ok_or_else(|| {
                ErrorInternalServerError("Backup metadata missing backup_key field")
            })?;
            let backup_bucket = metadata["backup_bucket"].as_str().ok_or_else(|| {
                ErrorInternalServerError("Backup metadata missing backup_bucket field")
            })?;

            let req = get_presigned_request(client, backup_bucket, backup_key, config).await?;
            let expires_at = (chrono::Utc::now()
                + chrono::Duration::seconds(config.backup_uri_timeout as i64))
            .to_rfc3339();

            Ok(BackupStatus::Completed {
                job_id,
                download_url: req.uri().to_string(),
                expires_at,
            })
        }
        Some("processing") => Ok(BackupStatus::Processing { job_id }),
        Some("failed") => Ok(BackupStatus::Failed {
            job_id,
            error: metadata["error"]
                .as_str()
                .unwrap_or("Backup process failed")
                .to_string(),
        }),
        _ => Ok(BackupStatus::Unknown { job_id }),
    }
}

/// Generates a pre-signed URL for downloading a backup file from S3.
///
/// Creates a temporary authenticated URL that allows downloading the backup file
/// without requiring AWS credentials. The URL expires after the configured timeout.
///
/// # Arguments
/// * `client` - AWS S3 client for making requests
/// * `object_key` - S3 object key of the backup file
/// * `config` - Application configuration containing S3 bucket and timeout settings
///
/// # Returns
/// * `Ok(PresignedRequest)` - Contains the pre-signed URL and its configuration
/// * `Err(Error)` - If URL generation fails, with detailed error logging:
///   - Invalid timeout value conversion
///   - Failed to create presigning configuration
///   - Object not found in S3
///   - Other S3 API errors
///
/// # Example URL Format
/// The generated URL will look like:
/// `https://<bucket>.s3.<region>.amazonaws.com/<key>?<signed-params>`
async fn get_presigned_request(
    client: &S3Client,
    backup_bucket: &str,
    object_key: &str,
    config: &config::Config,
) -> Result<PresignedRequest, Error> {
    let expires_in =
        tokio::time::Duration::from_secs(config.backup_uri_timeout.try_into().map_err(|e| {
            tracing::error!(
                error = %e,
                timeout = config.backup_uri_timeout,
                "Failed to convert backup URI timeout to u64"
            );
            ErrorInternalServerError("Invalid backup URI timeout value")
        })?);
    client
        .get_object()
        .bucket(backup_bucket)
        .key(object_key)
        .presigned(PresigningConfig::expires_in(expires_in).map_err(|e| {
            tracing::error!(
                error = %e,
                timeout_secs = expires_in.as_secs(),
                "Failed to create presigned URL config"
            );
            ErrorInternalServerError(format!("Failed to create presigned URL config: {}", e))
        })?)
        .await
        .map_err(|e| {
            if let Some(e) = e.as_service_error() {
                if e.is_invalid_object_state() || e.is_no_such_key() {
                    tracing::error!(
                        error = %e,
                        bucket = %backup_bucket,
                        key = %object_key,
                        "Backup file not found in S3"
                    );
                    return ErrorNotFound(format!("Backup file {} not found", object_key));
                }
            }
            tracing::error!(
                error = %e,
                bucket = %backup_bucket,
                key = %object_key,
                timeout_secs = expires_in.as_secs(),
                "Failed to generate presigned URL"
            );
            ErrorInternalServerError(format!("Failed to generate presigned URL: {}", e))
        })
}

/// Updates the backup status metadata in S3.
///
/// This function manages the status.json file for a backup job, updating it based on the backup result.
/// For successful backups, it sets the status to "completed" and adds a completion timestamp.
/// For failed backups, it sets the status to "failed", includes the error message, and adds a failure timestamp.
/// For processing backups, it sets the status to "processing" and adds a processing timestamp.
///
/// # Arguments
/// * `client` - AWS S3 client for accessing the bucket
/// * `bucket_name` - Name of the S3 bucket containing the backup metadata
/// * `metadata_key` - Full path to the status.json file in S3
/// * `result` - The backup operation result (Success, Failed with error message, or Processing)
/// * `namespace` - Namespace for the backup
///
/// # Returns
/// * `Ok(())` if the metadata was successfully updated
/// * `Err(Error)` if there was an error reading or writing the metadata file
///
/// # Example Status JSON Format
/// ```json
/// {
///     "status": "completed",
///     "backup_bucket": "my-bucket",
///     "backup_key": "path/to/backup.tar.gz",
///     "completed_at": "2024-03-21T15:30:00Z"
/// }
/// ```
/// or
/// ```json
/// {
///     "status": "failed",
///     "backup_bucket": "my-bucket",
///     "backup_key": "path/to/backup.tar.gz",
///     "error": "detailed error message",
///     "failed_at": "2024-03-21T15:30:00Z"
/// }
/// ```
/// or
/// ```json
/// {
///     "status": "processing",
///     "backup_bucket": "my-bucket",
///     "backup_key": "path/to/backup.tar.gz",
///     "processing_at": "2024-03-21T15:30:00Z"
/// }
/// ```
pub async fn update_backup_status(
    client: &S3Client,
    bucket_name: &str,
    metadata_key: &str,
    result: &BackupResult,
    namespace: &str,
) -> Result<(), Error> {
    tracing::debug!(
        bucket_name = %bucket_name,
        metadata_key = %metadata_key,
        result = ?result,
        namespace = %namespace,
        "Starting update_backup_status"
    );
    // Get the existing metadata first
    let existing_metadata = client
        .get_object()
        .bucket(bucket_name)
        .key(metadata_key)
        .send()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to get existing metadata from S3");
            ErrorInternalServerError(format!("Failed to get existing metadata from S3: {}", e))
        })?;
    tracing::debug!("Fetched existing metadata from S3");

    // Read the existing metadata
    let existing_bytes = existing_metadata.body.collect().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to read existing metadata from S3");
        ErrorInternalServerError(format!("Failed to read existing metadata from S3: {}", e))
    })?;
    tracing::debug!("Read existing metadata bytes from S3");
    let mut metadata: serde_json::Value = serde_json::from_slice(&existing_bytes.to_vec())
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to parse existing metadata");
            ErrorInternalServerError(format!("Failed to parse existing metadata: {}", e))
        })?;
    tracing::debug!(metadata = ?metadata, "Parsed existing metadata JSON");

    // Update the status based on the backup result
    if let Some(obj) = metadata.as_object_mut() {
        match result {
            BackupResult::Success => {
                // Derive backup tarball key from metadata_key
                let backup_key = if let Some(stripped) = metadata_key.strip_suffix("/status.json") {
                    format!("{stripped}/{namespace}.tar.gz")
                } else {
                    tracing::error!(metadata_key = %metadata_key, "metadata_key does not end with /status.json");
                    return Err(ErrorNotFound(format!(
                        "metadata_key does not end with /status.json: {}",
                        metadata_key
                    )));
                };

                // Check if the backup tarball exists in S3
                if !s3_object_exists(client, bucket_name, &backup_key).await? {
                    tracing::error!(bucket = %bucket_name, backup_key = %backup_key, "Backup tarball not found in S3");
                    return Err(ErrorNotFound(format!(
                        "Backup tarball not found in S3 at {}/{}",
                        bucket_name, backup_key
                    )));
                }
                tracing::info!(bucket = %bucket_name, backup_key = %backup_key, "Backup tarball found in S3");

                obj.insert("status".to_string(), json!("completed"));
                obj.insert(
                    "completed_at".to_string(),
                    json!(chrono::Utc::now().to_rfc3339()),
                );
                obj.insert("backup_bucket".to_string(), json!(bucket_name));
                obj.insert("backup_key".to_string(), json!(backup_key));
            }
            BackupResult::Failed(error) => {
                obj.insert("status".to_string(), json!("failed"));
                obj.insert("error".to_string(), json!(error));
                obj.insert(
                    "failed_at".to_string(),
                    json!(chrono::Utc::now().to_rfc3339()),
                );
            }
            BackupResult::Processing => {
                obj.insert("status".to_string(), json!("processing"));
                obj.insert(
                    "processing_at".to_string(),
                    json!(chrono::Utc::now().to_rfc3339()),
                );
            }
        }
        tracing::debug!(updated_metadata = ?obj, "Updated metadata object");
    }

    // Save updated metadata back to S3
    client
        .put_object()
        .bucket(bucket_name)
        .key(metadata_key)
        .body(ByteStream::from(metadata.to_string().into_bytes()))
        .content_type("application/json")
        .server_side_encryption(aws_sdk_s3::types::ServerSideEncryption::Aes256)
        .send()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to update backup metadata in S3");
            ErrorInternalServerError(format!("Failed to update backup metadata in S3: {}", e))
        })?;
    tracing::debug!("Successfully updated backup metadata in S3");
    Ok(())
}

/// Checks if an object exists in the specified S3 bucket and key.
///
/// This function sends a HEAD request to S3 to determine if the object exists.
/// Returns Ok(true) if the object exists, Ok(false) if it does not (404),
/// and Err(Error) for other errors.
///
/// # Arguments
/// * `client` - AWS S3 client
/// * `bucket` - Name of the S3 bucket
/// * `key` - Object key within the bucket
///
/// # Returns
/// * `Ok(true)` if the object exists
/// * `Ok(false)` if the object does not exist (404)
/// * `Err(Error)` for other errors
pub async fn s3_object_exists(client: &S3Client, bucket: &str, key: &str) -> Result<bool, Error> {
    tracing::debug!(bucket = %bucket, key = %key, "Checking if S3 object exists");
    match client.head_object().bucket(bucket).key(key).send().await {
        Ok(_) => {
            tracing::debug!(bucket = %bucket, key = %key, "S3 object exists");
            Ok(true)
        }
        Err(e) => {
            // If it's a not found error, return false, otherwise propagate error
            if e.to_string().contains("NotFound") {
                tracing::debug!(bucket = %bucket, key = %key, "S3 object does not exist (NotFound)");
                return Ok(false);
            }
            tracing::error!(error = %e, bucket = %bucket, key = %key, "Failed to check S3 object existence");
            Err(ErrorInternalServerError(format!(
                "Failed to check object existence: {}",
                e
            )))
        }
    }
}

/// Checks the Kubernetes Job status, updates the backup status in S3, and fetches the latest metadata.
///
/// This function ensures that the S3 status.json file accurately reflects the current state of the backup job in Kubernetes before returning the metadata.
/// It should be used in endpoints or logic where you want to present the most up-to-date backup status to users or clients.
///
/// # Arguments
/// * `kube_client` - Kubernetes client for querying the Job status
/// * `s3_client` - AWS S3 client for updating and fetching metadata
/// * `namespace` - Kubernetes namespace where the Job is running
/// * `job_id` - The backup job identifier (UUID)
/// * `bucket_name` - Name of the S3 bucket containing the backup metadata
/// * `metadata_key` - Full path to the status.json file in S3
///
/// # Returns
/// * `Ok(serde_json::Value)` - The latest backup metadata as JSON
/// * `Err(Error)` - If updating or fetching the metadata fails
///
pub async fn refresh_and_get_backup_metadata(
    kube_client: &kube::Client,
    s3_client: &S3Client,
    namespace: &str,
    job_id: &str,
    bucket_name: &str,
    metadata_key: &str,
) -> Result<serde_json::Value, Error> {
    tracing::debug!(
        namespace = %namespace,
        job_id = %job_id,
        bucket_name = %bucket_name,
        metadata_key = %metadata_key,
        "Starting refresh_and_get_backup_metadata"
    );
    // Get the current job status
    let job_status = get_job_status(kube_client, namespace, job_id).await;
    tracing::debug!(job_status = ?job_status, "Fetched job status");
    let backup_result = match job_status {
        JobStatus::Completed => BackupResult::Success,
        JobStatus::Failed => BackupResult::Failed("Backup job failed".to_string()),
        JobStatus::Processing => BackupResult::Processing,
        JobStatus::Unknown => BackupResult::Failed("Backup job status unknown".to_string()),
    };
    tracing::debug!(backup_result = ?backup_result, "Determined backup result");
    // Update the status.json in S3
    update_backup_status(
        s3_client,
        bucket_name,
        metadata_key,
        &backup_result,
        namespace,
    )
    .await?;
    tracing::debug!("Updated backup status in S3");
    // Fetch the latest metadata from S3
    let metadata = get_backup_metadata(s3_client, bucket_name, metadata_key).await?;
    tracing::debug!(metadata = ?metadata, "Fetched latest backup metadata from S3");
    Ok(metadata)
}
