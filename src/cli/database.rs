// Objects representing a user created local database
use chrono::prelude::*;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Database {
    pub name: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}
