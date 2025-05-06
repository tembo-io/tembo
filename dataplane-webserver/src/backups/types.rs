/// Represents the status of a backup job in the API response.
///
/// This enum is used to provide a consistent structure for backup status responses,
/// with each variant containing the necessary information for that status.
/// The enum is serialized with a `status` tag that determines the JSON structure.
///
/// # Variants
/// * `Completed` - Backup finished successfully, includes download URL and expiry
/// * `Processing` - Backup is currently in progress
/// * `Failed` - Backup failed with an error message
/// * `Unknown` - Backup status cannot be determined
///
/// # Example JSON
/// ```json
/// {
///     "status": "completed",
///     "job_id": "550e8400-e29b-41d4-a716-446655440000",
///     "download_url": "https://bucket.s3.region.amazonaws.com/path/to/backup?signed=params",
///     "expires_at": "2024-03-21T15:30:00Z"
/// }
/// ```
#[derive(Debug, serde::Serialize)]
#[serde(tag = "status")]
pub enum BackupStatus {
    /// Backup completed successfully
    #[serde(rename = "completed")]
    Completed {
        /// Unique identifier for the backup job
        job_id: String,
        /// Pre-signed S3 URL for downloading the backup
        download_url: String,
        /// ISO 8601 timestamp when the download URL expires
        expires_at: String,
    },
    /// Backup is currently in progress
    #[serde(rename = "processing")]
    Processing {
        /// Unique identifier for the backup job
        job_id: String,
    },
    /// Backup failed with an error
    #[serde(rename = "failed")]
    Failed {
        /// Unique identifier for the backup job
        job_id: String,
        /// Detailed error message explaining the failure
        error: String,
    },
    /// Backup status is unknown or invalid
    #[serde(rename = "unknown")]
    Unknown {
        /// Unique identifier for the backup job
        job_id: String,
    },
}

/// Represents the internal result of a backup operation.
///
/// This enum is used internally by the backup system to track the outcome
/// of backup operations before they are converted into API responses.
/// Unlike `BackupStatus`, this is a simpler enum focused on the operation
/// result rather than the API representation.
///
/// # Variants
/// * `Success` - Backup operation completed successfully
/// * `Processing` - Backup operation is currently in progress
/// * `Failed` - Backup operation failed with an error message
#[derive(Debug)]
pub enum BackupResult {
    /// Backup operation completed successfully
    Success,
    /// Backup operation is currently in progress
    Processing,
    /// Backup operation failed with an error message
    Failed(String),
}

/// Represents the connection information needed to connect to a database instance for backup.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub host: String,
    pub user: String,
    pub password: String,
    pub port: String,
}

/// Represents the status of a Kubernetes Job for backup processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    /// Job is still running
    Processing,
    /// Job completed successfully
    Completed,
    /// Job failed
    Failed,
    /// Job status is unknown or job does not exist
    Unknown,
}
