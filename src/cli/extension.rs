// Extensions are defined in Tembo Stacks and can be custom installed by users
// they are extensions to Postgres

use crate::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extension {
    pub name: Option<String>,
    pub version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub locations: Vec<ExtensionLocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionLocation {
    pub database: String,
    pub enabled: String,
    pub version: String,
}
