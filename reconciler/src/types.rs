use serde::{Deserialize, Serialize};

/// incoming message from control plane
#[derive(Debug, Deserialize, Serialize)]
pub struct CRUDevent {
    pub data_plane_id: String,
    pub event_id: String,
    pub message_type: String,
    pub body: EventBody,
}

pub enum UpdateEvent {
    ToggleExtension,
    UpdateInfra,
    InstallExtension,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EventBody {
    pub resource_type: String,
    pub resource_name: String,
    pub storage: Option<String>,
    pub memory: Option<String>,
    pub cpu: Option<String>,
    pub extensions: Option<Vec<Extension>>,
}

/// message returned to control plane
/// reports state of data plane
#[derive(Debug, Serialize, Deserialize)]
pub struct StateToControlPlane {
    pub data_plane_id: String, // unique identifier for the data plane
    pub event_id: String,      // pass through from event that triggered a data plane action
    pub state: State,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    pub connection: Option<String>,
    pub status: Status,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Status {
    Up,
    Deleted,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Extension {
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub schema: String,
}

impl Default for Extension {
    fn default() -> Self {
        Extension {
            name: "pg_stat_statements".to_owned(),
            version: "1.9".to_owned(),
            enabled: true,
            schema: "postgres".to_owned(),
        }
    }
}
