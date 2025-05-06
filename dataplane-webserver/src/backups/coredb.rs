use actix_web::{
    error::{ErrorInternalServerError, ErrorNotFound},
    Error,
};
use controller::apis::coredb_types::CoreDB;
use kube::{api::Api, Client as KubeClient};

/// Fetches the CoreDB custom resource in the given namespace from the Kubernetes cluster.
///
/// # Arguments
/// * `kube_client` - Reference to the Kubernetes client used to interact with the cluster.
/// * `namespace` - The namespace in which to look for the CoreDB resource. The CoreDB resource is expected to have the same name as the namespace.
///
/// # Returns
/// * `Ok(CoreDB)` if the resource is found successfully.
/// * `Err(Error)` if the resource cannot be fetched from the cluster.
///
/// # Errors
/// Returns an Actix `ErrorInternalServerError` if the CoreDB resource cannot be retrieved from the cluster.
pub async fn fetch_coredb(kube_client: &KubeClient, namespace: &str) -> Result<CoreDB, Error> {
    let coredb_api = Api::namespaced(kube_client.clone(), namespace);
    let coredb = coredb_api
        .get(namespace)
        .await
        .map_err(|e| ErrorNotFound(format!("Failed to get CoreDB or CoreDB not found: {}", e)))?;
    Ok(coredb)
}

/// Retrieves and parses the S3 backup path from a CoreDB instance's backup configuration.
///
/// # Arguments
/// * `coredb` - Reference to the CoreDB resource
///
/// # Returns
/// * `Ok((String, String))` - Tuple of (bucket_name, object_path)
/// * `Err(Error)` - If backup configuration is missing or invalid
pub fn get_backup_path_from_coredb(coredb: &CoreDB) -> Result<(String, String), Error> {
    let backup = &coredb.spec.backup;
    if let Some(destination_path) = &backup.destinationPath {
        tracing::info!(
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

/// Returns the value of the `spec.storage` field from a CoreDB resource as a String.
///
/// # Arguments
/// * `coredb` - Reference to the CoreDB resource
///
/// # Returns
/// * `Ok(String)` containing the storage size (e.g., "10Gi") if present.
/// * `Err(Error)` if the field is missing.
pub fn get_storage_size_from_coredb(coredb: &CoreDB) -> Result<String, Error> {
    Ok(coredb.spec.storage.0.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::error::ErrorInternalServerError;

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
}
