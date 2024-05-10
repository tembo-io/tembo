use crate::{apis::coredb_types::CoreDB, Context};
use chrono::{DateTime, Utc};
use kube::runtime::controller::Action;
use std::sync::Arc;

const WAL_ARCHIVE_STATUS_QUERY: &str = r#"
SELECT
    to_char((last_archived_time::timestamp), 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"')
FROM pg_stat_archiver;
"#;

// WalArchiveStatus struct to hold the status of the WAL archive
// for now we are only interested in the last archived WAL time
#[derive(Debug, Clone)]
pub struct WalArchiveStatus {
    pub last_archived_time: Option<DateTime<Utc>>,
}

/// get_wal_archive_status queries the pg_stat_archiver table to get the status of the WAL archive
/// Returns a WalArchiveStatus struct
pub async fn get_wal_archive_status(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<WalArchiveStatus, Action> {
    let query = cdb
        .psql(
            WAL_ARCHIVE_STATUS_QUERY.to_string(),
            "postgres".to_string(),
            ctx.clone(),
        )
        .await?;

    let last_archived_time = query.get_field(0).and_then(|s| parse_date_time(&s));

    // Convert the query result into a WalArchiveStatus struct
    let wal_archive_status = WalArchiveStatus { last_archived_time };

    Ok(wal_archive_status)
}

/// Parses a date-time string in ISO 8601 format and converts it to a `DateTime<Utc>`.
fn parse_date_time(date_str: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(date_str)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

// Find status of the last time a WAL archive was successful and return the date
pub async fn reconcile_last_archive_status(
    cdb: &CoreDB,
    ctx: Arc<Context>,
) -> Result<Option<DateTime<Utc>>, Action> {
    // Get the WAL archive status from the get_wal_archive_status function
    let wal_archive_status = get_wal_archive_status(cdb, ctx.clone()).await?;

    // If the last archived WAL time is None, return None
    if wal_archive_status.last_archived_time.is_none() {
        return Ok(None);
    }

    // If wal_archive_status.last_archived_time is Some, return the last archived time
    Ok(wal_archive_status.last_archived_time)
}
