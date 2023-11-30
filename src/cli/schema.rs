//! Objects representing a user created local database
use chrono::prelude::*;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Schema {
    pub name: String,
    pub created_at: DateTime<Utc>,
}
