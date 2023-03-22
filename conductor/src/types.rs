use serde::{Deserialize, Serialize};

use crate::coredb_crd as crd;
use crate::types;

/// incoming message from control plane
#[derive(Debug, Deserialize, Serialize)]
pub struct CRUDevent {
    pub data_plane_id: String,
    pub event_id: String,
    pub event_type: Event,
    pub dbname: String,
    pub spec: crd::CoreDBSpec,
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
    pub spec: Option<crd::CoreDBSpec>,
    pub connection: Option<types::ConnectionInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}
