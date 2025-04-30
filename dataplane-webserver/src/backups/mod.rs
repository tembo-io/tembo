pub mod s3;
pub mod types;

use crate::{
    backups::{s3::update_backup_status, types::BackupResult},
    config::Config,
};
use actix_web::{error::ErrorInternalServerError, Error};
use aws_sdk_s3::Client as S3Client;
use controller::apis::coredb_types::CoreDB;
use k8s_openapi::api::core::v1::{Namespace, Pod};
use kube::{
    api::{Api, AttachParams, ListParams},
    Client as KubeClient,
};
use tokio::io::AsyncReadExt;

const TEMBACK_INSTALL_DIR: &str = "/var/lib/postgresql/data";
const TEMBACK_INSTALL_CMD_TEMPLATE: &str = "curl -L https://github.com/tembo-io/temback/releases/download/{version}/temback-{version}-linux-amd64.tar.gz | tar -C {install_dir} --strip-components=1 -zxf - temback-{version}-linux-amd64/temback && chmod +x {install_dir}/temback";

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
) -> Result<(), Error> {
    // Log the start of the backup process
    tracing::info!(
        organization_id = %org_id,
        instance_id = %instance_id,
        job_id = %job_id,
        "Starting database backup"
    );

    // Perform the actual backup
    let backup_result =
        run_backup_on_instance_pod(kube_client, &namespace, &bucket_name, &bucket_path, config)
            .await?;

    // Define the metadata key
    let metadata_key = format!("{bucket_path}/status.json");

    // Update the status based on the backup result
    update_backup_status(
        client,
        &bucket_name,
        &metadata_key,
        &backup_result,
        &namespace,
    )
    .await?;

    match backup_result {
        BackupResult::Success => {
            tracing::info!(
                organization_id = %org_id,
                instance_id = %instance_id,
                job_id = %job_id,
                "Backup completed successfully"
            );
            Ok(())
        }
        BackupResult::Failed(error) => {
            tracing::error!(
                error = %error,
                organization_id = %org_id,
                instance_id = %instance_id,
                job_id = %job_id,
                "Backup failed"
            );
            Err(ErrorInternalServerError(error))
        }
    }
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

/// Retrieves and parses the S3 backup path from a CoreDB instance's backup configuration.
///
/// This function performs the following steps:
/// 1. Fetches the CoreDB instance from Kubernetes using the namespace
/// 2. Extracts the destinationPath from the backup configuration
/// 3. Parses the S3 URI to separate bucket name and object path
///
/// # Arguments
/// * `kube_client` - Kubernetes client for accessing the CoreDB resource
/// * `namespace` - Namespace where the CoreDB instance is located
///
/// # Returns
/// * `Ok((String, String))` - Tuple of (bucket_name, object_path)
/// * `Err(Error)` - If:
///   - CoreDB instance not found
///   - Backup configuration missing
///   - Invalid S3 URI format
///   - Missing destination path
///
/// # Example URI Format
/// The expected S3 URI format is: `s3://bucket-name/path/to/directory`
/// For example: `s3://my-bucket/backups/instance-1`
pub async fn get_backup_path_from_coredb(
    kube_client: &KubeClient,
    namespace: &str,
) -> Result<(String, String), Error> {
    let coredb_api: Api<CoreDB> = Api::namespaced(kube_client.clone(), namespace);
    let coredb_name = namespace;

    // Get the CoreDBSpec from the Kubernetes API
    let coredb = coredb_api
        .get(coredb_name)
        .await
        .map_err(|e| ErrorInternalServerError(format!("Failed to get CoreDB: {}", e)))?;

    // Access the backup configuration from the spec
    let backup = &coredb.spec.backup;

    // Get the destination path from the backup configuration
    if let Some(destination_path) = &backup.destinationPath {
        tracing::info!(
            namespace = %namespace,
            destination_path = %destination_path,
            "Found backup destination path"
        );

        // Parse the S3 URI (format: s3://bucket-name/path/to/directory)
        if let Some(path_without_prefix) = destination_path.strip_prefix("s3://") {
            // Split by the first slash to separate bucket and path
            if let Some(first_slash_pos) = path_without_prefix.find('/') {
                let bucket_name = &path_without_prefix[0..first_slash_pos];
                let object_path = &path_without_prefix[first_slash_pos + 1..];

                tracing::info!(
                    bucket = %bucket_name,
                    path = %object_path,
                    "Parsed S3 URI components"
                );

                Ok((bucket_name.to_string(), object_path.to_string()))
            } else {
                // No path component, just a bucket
                Ok((path_without_prefix.to_string(), "".to_string()))
            }
        } else {
            Err(ErrorInternalServerError(format!(
                "Destination path is not a valid S3 URI: {}",
                destination_path
            )))
        }
    } else {
        Err(ErrorInternalServerError(
            "CoreDB backup configuration does not contain a destination path".to_string(),
        ))
    }
}

/// Install temback binary in the specified pod if it doesn't exist or version doesn't match.
///
/// # Arguments
/// * `pods_api` - Kubernetes Pod API
/// * `pod_name` - Name of the pod to install temback in
/// * `config` - Application configuration containing temback version
///
/// # Returns
/// * `Ok(())` if installation succeeds or binary already exists with correct version
/// * `Err(Error)` if installation fails
async fn install_temback(
    pods_api: &Api<Pod>,
    pod_name: &str,
    config: &Config,
) -> Result<(), Error> {
    let attach_params = AttachParams::default()
        .container("postgres")
        .stderr(true)
        .stdout(true)
        .stdin(false);

    // Try to get current version
    let version_cmd = vec!["/var/lib/postgresql/data/temback", "--version"];
    let mut needs_install = true;

    tracing::debug!(
        pod = %pod_name,
        command = ?version_cmd,
        "Checking temback version"
    );

    match pods_api.exec(pod_name, version_cmd, &attach_params).await {
        Ok(mut version_output) => {
            let mut version_stdout = String::new();
            if let Some(mut stdout) = version_output.stdout() {
                stdout
                    .read_to_string(&mut version_stdout)
                    .await
                    .map_err(|e| {
                        ErrorInternalServerError(format!("Failed to read version output: {}", e))
                    })?;
            }

            // Parse version from output format (eg: "temback v0.1.1 (0a6689c)")
            if let Some(version) = version_stdout.split_whitespace().nth(1) {
                if version == config.temback_version {
                    tracing::debug!(
                        pod = %pod_name,
                        version = %version,
                        "Found matching temback version"
                    );
                    needs_install = false;
                } else {
                    tracing::info!(
                        pod = %pod_name,
                        current_version = %version,
                        desired_version = %config.temback_version,
                        "temback version mismatch, will reinstall"
                    );
                }
            }
        }
        Err(e) => {
            tracing::info!(
                error = %e,
                pod = %pod_name,
                "Failed to get temback version, will install"
            );
        }
    }

    if needs_install {
        tracing::info!(
            pod = %pod_name,
            version = %config.temback_version,
            "Installing temback..."
        );

        let install_cmd_str = TEMBACK_INSTALL_CMD_TEMPLATE
            .replace("{version}", &config.temback_version)
            .replace("{install_dir}", TEMBACK_INSTALL_DIR);
        let install_cmd = vec!["sh", "-c", &install_cmd_str];

        tracing::debug!(
            pod = %pod_name,
            command = ?install_cmd,
            "Installing temback"
        );

        let mut install_output = pods_api
            .exec(pod_name, install_cmd, &attach_params)
            .await
            .map_err(|e| ErrorInternalServerError(format!("Failed to install temback: {}", e)))?;

        // Wait for install to finish by reading all output
        let mut _out = String::new();
        if let Some(mut stdout) = install_output.stdout() {
            stdout.read_to_string(&mut _out).await.ok();
        }
        let mut _err = String::new();
        if let Some(mut stderr) = install_output.stderr() {
            stderr.read_to_string(&mut _err).await.ok();
        }

        // Verify the binary exists and check its version
        let verify_cmd = vec!["/var/lib/postgresql/data/temback", "--version"];
        let mut verify_result = pods_api
            .exec(pod_name, verify_cmd, &attach_params)
            .await
            .map_err(|e| {
                ErrorInternalServerError(format!("Failed to verify temback installation: {}", e))
            })?;

        let mut verify_stdout = String::new();
        if let Some(mut stdout) = verify_result.stdout() {
            stdout
                .read_to_string(&mut verify_stdout)
                .await
                .map_err(|e| {
                    ErrorInternalServerError(format!("Failed to read verification output: {}", e))
                })?;
        }

        if !verify_stdout.contains(&config.temback_version) {
            return Err(ErrorInternalServerError(format!(
                "Failed to install correct temback version. Got output: {}",
                verify_stdout
            )));
        }

        tracing::info!(
            pod = %pod_name,
            version = %config.temback_version,
            "Successfully installed temback"
        );
    }

    Ok(())
}

/// Executes a backup using temback in the database pod.
///
/// This function performs the following steps:
/// 1. Finds the primary database pod in the namespace using CloudNativePG labels
/// 2. Checks for and installs temback binary if not present
/// 3. Executes temback command to perform backup directly to S3
///
/// # Arguments
/// * `kube_client` - Kubernetes client for pod operations
/// * `namespace` - Namespace where the database pod is running
/// * `bucket_name` - S3 bucket to store the backup
/// * `bucket_path` - Base path in the S3 bucket
/// * `config` - Application configuration containing temback version
///
/// # Returns
/// * `Ok(BackupResult)` containing Success or Failed with error message
/// * `Err(Error)` if any step fails catastrophically
async fn run_backup_on_instance_pod(
    kube_client: &KubeClient,
    namespace: &str,
    bucket_name: &str,
    bucket_path: &str,
    config: &Config,
) -> Result<BackupResult, Error> {
    // Find the primary database pod
    let pods_api: Api<Pod> = Api::namespaced(kube_client.clone(), namespace);
    let label_selector = format!("cnpg.io/cluster={},cnpg.io/instanceRole=primary", namespace);
    let params = ListParams::default().labels(&label_selector);

    let pods = pods_api
        .list(&params)
        .await
        .map_err(|e| ErrorInternalServerError(format!("Failed to list pods: {}", e)))?;

    if pods.items.is_empty() {
        return Ok(BackupResult::Failed(
            "No primary database pod found".to_string(),
        ));
    }

    let pod = &pods.items[0];
    let pod_name = pod
        .metadata
        .name
        .as_ref()
        .ok_or_else(|| ErrorInternalServerError("Pod has no name"))?;

    tracing::info!(
        namespace = %namespace,
        pod = %pod_name,
        "Found primary database pod for backup"
    );

    // Install temback if needed
    if let Err(e) = install_temback(&pods_api, pod_name, config).await {
        return Ok(BackupResult::Failed(e.to_string()));
    }

    // Execute temback command in the pod
    let attach_params = AttachParams::default()
        .container("postgres")
        .stderr(true)
        .stdout(true)
        .stdin(false);

    let backup_cmd = vec![
        "/var/lib/postgresql/data/temback",
        "--name",
        namespace,
        "--compress",
        "--clean",
        "--cd",
        "/tmp",
        "--bucket",
        &bucket_name,
        "--dir",
        &bucket_path,
    ];

    tracing::debug!(
        pod = %pod_name,
        command = ?backup_cmd,
        "Executing temback backup command"
    );

    let mut backup_result = pods_api
        .exec(pod_name, backup_cmd, &attach_params)
        .await
        .map_err(|e| {
            ErrorInternalServerError(format!("Failed to execute backup command: {}", e))
        })?;

    // Collect both stdout and stderr
    let mut stdout_msg = String::new();
    let mut stderr_msg = String::new();

    if let Some(mut stdout) = backup_result.stdout() {
        stdout.read_to_string(&mut stdout_msg).await.map_err(|e| {
            ErrorInternalServerError(format!("Failed to read command output: {}", e))
        })?;
    }

    if let Some(mut stderr) = backup_result.stderr() {
        stderr
            .read_to_string(&mut stderr_msg)
            .await
            .map_err(|e| ErrorInternalServerError(format!("Failed to read error output: {}", e)))?;
    }

    // Log the output regardless of success/failure
    if !stdout_msg.is_empty() {
        tracing::info!(
            output = %stdout_msg,
            pod = %pod_name,
            "Backup command stdout"
        );
    }

    // If we got any stderr output, consider it a failure
    if !stderr_msg.is_empty() {
        tracing::error!(
            error = %stderr_msg,
            pod = %pod_name,
            "Backup command failed with error output"
        );
        return Ok(BackupResult::Failed(format!(
            "Backup command failed: {}",
            stderr_msg
        )));
    }

    tracing::info!(
        namespace = %namespace,
        pod = %pod_name,
        bucket = %bucket_name,
        bucket_path = %bucket_path,
        "Successfully ran backup and uploaded to S3"
    );

    Ok(BackupResult::Success)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::error::ErrorInternalServerError;

    fn redact_password_arg(cmd: &[&str]) -> Vec<String> {
        let mut redacted = cmd.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        if let Some(pass_index) = redacted.iter().position(|s| s == "--pass") {
            if pass_index + 1 < redacted.len() {
                redacted[pass_index + 1] = "[REDACTED]".to_string();
            }
        }
        redacted
    }

    /// Helper function to test S3 URI parsing logic directly
    fn parse_s3_uri(uri: &str) -> Result<(String, String), Error> {
        if let Some(path_without_prefix) = uri.strip_prefix("s3://") {
            if let Some(first_slash_pos) = path_without_prefix.find('/') {
                let bucket_name = &path_without_prefix[0..first_slash_pos];
                let object_path = &path_without_prefix[first_slash_pos + 1..];
                Ok((bucket_name.to_string(), object_path.to_string()))
            } else {
                Ok((path_without_prefix.to_string(), "".to_string()))
            }
        } else {
            Err(ErrorInternalServerError(format!(
                "Destination path is not a valid S3 URI: {}",
                uri
            )))
        }
    }

    #[test]
    fn test_s3_uri_parsing() {
        // Test case from the example configuration
        let result =
            parse_s3_uri("s3://cdb-plat-use1-dev-instance-backups/v2/sorely-adapted-redpoll")
                .expect("Should parse valid URI");
        assert_eq!(result.0, "cdb-plat-use1-dev-instance-backups");
        assert_eq!(result.1, "v2/sorely-adapted-redpoll");

        // Test bucket only
        let result = parse_s3_uri("s3://my-bucket").expect("Should parse bucket-only URI");
        assert_eq!(result.0, "my-bucket");
        assert_eq!(result.1, "");

        // Test multiple path segments
        let result = parse_s3_uri("s3://bucket/path/to/backup/dir")
            .expect("Should parse multi-segment path");
        assert_eq!(result.0, "bucket");
        assert_eq!(result.1, "path/to/backup/dir");

        // Test invalid URI (no s3:// prefix)
        assert!(parse_s3_uri("invalid-uri").is_err());
        assert!(parse_s3_uri("http://wrong-protocol").is_err());
    }

    #[test]
    fn test_redact_password_arg() {
        let cmd = [
            "/var/lib/postgresql/data/temback",
            "--name",
            "testns",
            "--pass",
            "supersecret",
            "--host",
            "localhost",
        ];
        let redacted = redact_password_arg(&cmd);
        assert_eq!(redacted[3], "--pass");
        assert_eq!(redacted[4], "[REDACTED]");
        // Ensure other args are unchanged
        assert_eq!(redacted[2], "testns");
        assert_eq!(redacted[5], "--host");
        assert_eq!(redacted[6], "localhost");
    }
}
