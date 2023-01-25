use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct CRUDevent {
    pub data_plane_id: String,
    pub event_id: String,
    pub message_type: String,
    pub body: EventBody,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EventBody {
    pub resource_type: String,
    pub resource_name: String,
    pub storage: Option<String>,
    pub memory: Option<String>,
    pub cpu: Option<String>,
    pub extensions: Option<Vec<String>>,
}
