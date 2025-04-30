use crate::{
    backups::find_instance_namespace, backups::get_backup_path_from_coredb,
    backups::perform_backup_task, backups::s3::determine_backup_status,
    backups::s3::get_backup_metadata, config,
};
use actix_web::{
    error::ErrorInternalServerError, get, post, web, Error, HttpRequest, HttpResponse,
};
use aws_config::meta::region::RegionProviderChain;
use aws_config::BehaviorVersion;
use aws_sdk_s3::{config::Region, Client};
use aws_smithy_types::byte_stream::ByteStream;
use kube::Client as KubeClient;
use serde_json::json;
use uuid::Uuid;

/// Initiates an asynchronous backup process for a database instance.
///
/// This endpoint triggers a backup operation and immediately returns a job ID while the backup
/// continues in the background. The backup status can be monitored using the job ID.
///
/// # Path Parameters
/// * `org_id` - Organization identifier (alphanumeric or underscore)
/// * `inst_id` - Instance identifier (alphanumeric or underscore)
///
/// # Response
/// * `202 Accepted` - Returns JSON with job_id and initial processing status
/// * `400 Bad Request` - If org_id or inst_id contain invalid characters
/// * `500 Internal Server Error` - If metadata creation in S3 fails
///
/// Example success response:
/// ```json
/// {
///     "job_id": "550e8400-e29b-41d4-a716-446655440000",
///     "status": "processing"
/// }
/// ```
#[post("/backup")]
pub async fn trigger_instance_backup(
    _req: HttpRequest,
    path: web::Path<(String, String)>,
    config: web::Data<config::Config>,
) -> Result<HttpResponse, Error> {
    let (org_id, inst_id) = path.into_inner();
    if !crate::routes::secrets::is_valid_id(&org_id)
        || !crate::routes::secrets::is_valid_id(&inst_id)
    {
        return Ok(HttpResponse::BadRequest()
            .json("org_id and instance_id must be alphanumeric or underscore only"));
    }

    // Generate a unique job ID (UUID v4)
    let job_id = Uuid::new_v4().to_string();

    // Create S3 client
    let region_provider = RegionProviderChain::default_provider()
        .or_else(Region::new(config.backup_bucket_region.clone()));
    let shared_config = aws_config::defaults(BehaviorVersion::v2025_01_17())
        .region(region_provider)
        .load()
        .await;
    let client = Client::new(&shared_config);

    // Create Kubernetes client to get the CoreDBSpec
    let kube_client = KubeClient::try_default().await.map_err(|e| {
        ErrorInternalServerError(format!("Failed to create Kubernetes client: {}", e))
    })?;

    // Find the namespace for this instance
    let namespace = find_instance_namespace(&kube_client, &org_id, &inst_id).await?;
    // Get the backup path from CoreDB spec
    let (backup_bucket_name, backup_base_path) =
        get_backup_path_from_coredb(&kube_client, &namespace).await?;

    let backup_path = format!("{backup_base_path}/temback/{job_id}");

    // Create job metadata
    let metadata = json!({
        "job_id": job_id,
        "org_id": org_id.clone(),
        "instance_id": inst_id.clone(),
        "status": "processing",
        "created_at": chrono::Utc::now().to_rfc3339()
    });

    // Save metadata to S3
    let metadata_key = format!("{backup_base_path}/temback/{job_id}/status.json");
    let put_result = client
        .put_object()
        .bucket(backup_bucket_name.clone())
        .key(&metadata_key)
        .body(ByteStream::from(metadata.to_string().into_bytes()))
        .content_type("application/json")
        .server_side_encryption(aws_sdk_s3::types::ServerSideEncryption::Aes256)
        .send()
        .await;

    if let Err(e) = put_result {
        tracing::error!("S3 put_object error: {:?}", e);
        return Err(ErrorInternalServerError(format!(
            "Failed to save backup metadata to S3: {e:?}",
        )));
    }

    // Start the backup process in the background
    let s3_client = client.clone();
    let k8s_client = kube_client.clone();
    let spawn_org_id = org_id.clone();
    let spawn_inst_id = inst_id.clone();
    let spawn_job_id = job_id.clone();
    let spawn_bucket_name = backup_bucket_name;
    let spawn_base_path = backup_path.clone();
    let spawn_config = config.clone();

    actix_web::rt::spawn(async move {
        if let Err(e) = perform_backup_task(
            spawn_org_id.clone(),
            spawn_inst_id.clone(),
            spawn_job_id.clone(),
            &s3_client,
            &k8s_client,
            spawn_bucket_name,
            spawn_base_path,
            namespace.clone(),
            &spawn_config,
        )
        .await
        {
            tracing::error!(
                error = %e,
                namespace = %namespace,
                org_id = %spawn_org_id,
                instance_id = %spawn_inst_id,
                job_id = %spawn_job_id,
                "Failed to perform backup task"
            );
        }
    });

    // Return the job ID immediately
    Ok(HttpResponse::Accepted().json(json!({
        "job_id": job_id,
        "status": "processing"
    })))
}

/// Retrieves the current status of a backup job.
///
/// This endpoint checks the status of a backup job and returns a structured response based on
/// the current state of the backup operation. The response format is determined by the
/// `BackupStatus` enum, which ensures consistent status reporting.
///
/// # Path Parameters
/// * `org_id` - Organization identifier (alphanumeric or underscore)
/// * `inst_id` - Instance identifier (alphanumeric or underscore)
/// * `job_id` - Backup job identifier (UUID)
///
/// # Response Format
/// Returns a JSON object with a `status` field that determines the structure:
///
/// For completed backups:
/// ```json
/// {
///     "status": "completed",
///     "job_id": "550e8400-e29b-41d4-a716-446655440000",
///     "download_url": "https://bucket.s3.region.amazonaws.com/path/to/backup?signed=params",
///     "expires_at": "2024-03-21T15:30:00Z"
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
///     "error": "detailed error message"
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
///
/// # Status Codes
/// * `200 OK` - Successfully retrieved the backup status
/// * `400 Bad Request` - If org_id or inst_id contain invalid characters
/// * `404 Not Found` - If the backup job doesn't exist
/// * `500 Internal Server Error` - If there's an error accessing S3 or parsing metadata
#[get("/backup/{job_id}")]
pub async fn get_backup_status(
    _req: HttpRequest,
    path: web::Path<(String, String, String)>,
    config: web::Data<config::Config>,
) -> Result<HttpResponse, Error> {
    let (org_id, inst_id, job_id) = path.into_inner();
    if !crate::routes::secrets::is_valid_id(&org_id)
        || !crate::routes::secrets::is_valid_id(&inst_id)
    {
        return Ok(HttpResponse::BadRequest()
            .json("org_id and instance_id must be alphanumeric or underscore only"));
    }

    // Create S3 client
    let region_provider = RegionProviderChain::default_provider()
        .or_else(Region::new(config.backup_bucket_region.clone()));
    let shared_config = aws_config::defaults(BehaviorVersion::v2025_01_17())
        .region(region_provider)
        .load()
        .await;
    let client = Client::new(&shared_config);

    // Create Kubernetes client and get backup path
    let kube_client = KubeClient::try_default().await.map_err(|e| {
        ErrorInternalServerError(format!("Failed to create Kubernetes client: {}", e))
    })?;
    let namespace = find_instance_namespace(&kube_client, &org_id, &inst_id).await?;
    let (backup_bucket_name, backup_base_path) =
        get_backup_path_from_coredb(&kube_client, &namespace).await?;

    // Get and parse the backup metadata
    let metadata_key = format!("{backup_base_path}/temback/{job_id}/status.json");
    let metadata = match get_backup_metadata(&client, &backup_bucket_name, &metadata_key).await {
        Ok(metadata) => metadata,
        Err(e) => {
            if e.to_string().contains("not found") {
                return Ok(HttpResponse::NotFound().json(json!({
                    "error": format!("Backup job with ID {} not found", job_id)
                })));
            }
            return Err(e);
        }
    };

    // Determine and return the backup status
    let status = determine_backup_status(metadata, job_id, &client, &config).await?;
    Ok(HttpResponse::Ok().json(status))
}
