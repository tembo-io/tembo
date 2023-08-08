use serde::{Deserialize, Serialize};

use crate::types;
use controller::apis::coredb_types::{CoreDBSpec, CoreDBStatus};

/// incoming message from control plane
#[derive(Debug, Deserialize, Serialize)]
pub struct CRUDevent {
    pub organization_name: String,
    pub data_plane_id: String,
    pub event_id: String,
    pub event_type: Event,
    pub dbname: String,
    pub spec: Option<CoreDBSpec>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
}

/// message returned to control plane
/// reports state of data plane
#[derive(Debug, Serialize, Deserialize)]
pub struct StateToControlPlane {
    pub data_plane_id: String, // unique identifier for the data plane
    pub event_id: String,      // pass through from event that triggered a data plane action
    pub event_type: Event,     // pass through from event that triggered a data plane action
    pub spec: Option<CoreDBSpec>,
    pub status: Option<CoreDBStatus>,
    pub connection: Option<types::ConnectionInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}
