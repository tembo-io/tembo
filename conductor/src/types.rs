use serde::{Deserialize, Serialize};

use crate::types;
use controller::apis::coredb_types::{CoreDBSpec, CoreDBStatus};

/// incoming message from control plane
#[derive(Debug, Deserialize, Serialize)]
pub struct CRUDevent {
    pub data_plane_id: String,
    pub org_id: String,
    pub inst_id: String,
    pub event_type: Event,
    pub namespace: String,
    pub backups_read_path: Option<String>,
    pub backups_write_path: Option<String>,
    pub spec: Option<CoreDBSpec>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Event {
    Create,
    Created,
    Error,
    Update,
    Updated,
    Restart,
    Restarted,
    Stop,
    StopComplete,
    Delete,
    Deleted,
    Start,
    Started,
    Restore,
    Restored,
    ScheduleDeletion,
    ScheduleDeletionComplete,
}

/// message returned to control plane
/// reports state of data plane
#[derive(Debug, Serialize, Deserialize)]
pub struct StateToControlPlane {
    pub data_plane_id: String, // unique identifier for the data plane
    pub event_type: Event,
    pub org_id: String,
    pub inst_id: String,
    pub spec: Option<CoreDBSpec>,
    pub status: Option<CoreDBStatus>,
    pub connection: Option<types::ConnectionInfo>,
}

#[derive(Debug)]
pub struct OrgInstId {
    pub org_id: String,
    pub inst_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub host: String,
    pub pooler_host: Option<String>,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub app_user: String,
    pub app_password: String,
}
