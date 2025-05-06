pub mod coredb;
pub mod job;
pub mod s3;
pub mod temback;
pub mod types;
pub use job::create_backup_job;

use crate::{
    backups::{s3::update_backup_status, types::BackupResult},
    config::Config,
};
use actix_web::{error::ErrorInternalServerError, Error};
use aws_sdk_s3::Client as S3Client;
use controller::apis::coredb_types::CoreDB;
use k8s_openapi::api::core::v1::Namespace;
use kube::{
    api::{Api, ListParams},
    Client as KubeClient,
};

/// Executes a backup task for a specific database instance and updates its status in S3.
///
/// This function runs asynchronously and performs the following steps:
/// 1. Logs the start of the backup process
/// 2. Executes backup using temback
/// 3. Updates the job metadata in S3 with completion status
///
/// # Arguments
/// * `org_id` - Organization identifier
/// * `instance_id` - Database instance identifier
/// * `job_id` - Unique identifier for this backup job
/// * `client` - Initialized AWS S3 client
/// * `kube_client` - Kubernetes client
/// * `bucket_name` - Name of the S3 bucket
/// * `bucket_path` - Base path in the bucket
/// * `namespace` - Kubernetes namespace
/// * `config` - Application configuration
/// * `coredb` - CoreDB object
///
/// # Returns
/// * `Ok(())` if the backup process and metadata updates complete successfully
/// * `Err(Error)` if any step fails, with a descriptive error message
#[allow(clippy::too_many_arguments)]
pub async fn perform_backup_task(
    org_id: String,
    instance_id: String,
    job_id: String,
    client: &S3Client,
    kube_client: &KubeClient,
    bucket_name: String,
    bucket_path: String,
    namespace: String,
    config: &Config,
    coredb: &CoreDB,
) -> Result<(), Error> {
    // Log the start of the backup process
    tracing::info!(
        organization_id = %org_id,
        instance_id = %instance_id,
        job_id = %job_id,
        "Starting database backup (via Job)"
    );

    // Get the storage size from CoreDB
    let storage_size = coredb::get_storage_size_from_coredb(coredb)?;
    let temback_image_name = &config.temback_image;
    let temback_image_version = &config.temback_version;
    let temback_image = format!("{temback_image_name}:{temback_image_version}");
    // Create the backup Job
    create_backup_job(
        kube_client,
        &namespace,
        &job_id,
        &bucket_name,
        &bucket_path,
        &storage_size,
        &temback_image,
    )
    .await?;

    // Define the metadata key
    let metadata_key = format!("{bucket_path}/status.json");

    // Update the status to processing (Job creation is async)
    update_backup_status(
        client,
        &bucket_name,
        &metadata_key,
        &BackupResult::Processing,
        &namespace,
    )
    .await?;

    tracing::info!(
        organization_id = %org_id,
        instance_id = %instance_id,
        job_id = %job_id,
        "Backup Job created successfully"
    );
    Ok(())
}

/// Find the Kubernetes namespace for a given organization and instance.
///
/// Looks up the namespace using Kubernetes labels for organization_id and instance_id.
/// There should only be one matching namespace.
///
/// # Arguments
/// * `kube_client` - Kubernetes client
/// * `org_id` - Organization identifier
/// * `inst_id` - Instance identifier
///
/// # Returns
/// * `Ok(String)` - Name of the namespace
/// * `Err(Error)` - If namespace lookup fails or no matching namespace is found
pub async fn find_instance_namespace(
    kube_client: &KubeClient,
    org_id: &str,
    inst_id: &str,
) -> Result<String, Error> {
    let namespaces_api: Api<Namespace> = Api::all(kube_client.clone());

    let label_selector = format!(
        "tembo.io/organization_id={},tembo.io/instance_id={}",
        org_id, inst_id
    );

    let params = ListParams::default().labels(&label_selector);

    let namespaces = namespaces_api
        .list(&params)
        .await
        .map_err(|e| ErrorInternalServerError(format!("Failed to list namespaces: {}", e)))?;

    if namespaces.items.is_empty() {
        return Err(ErrorInternalServerError(format!(
            "No namespace found for organization_id={} and instance_id={}",
            org_id, inst_id
        )));
    }

    // Get the first matching namespace (should only be one)
    let namespace = namespaces.items[0]
        .metadata
        .name
        .as_ref()
        .ok_or_else(|| ErrorInternalServerError("Namespace has no name".to_string()))?
        .clone();

    tracing::info!(
        organization_id = %org_id,
        instance_id = %inst_id,
        namespace = %namespace,
        "Found namespace for backup"
    );

    Ok(namespace)
}
