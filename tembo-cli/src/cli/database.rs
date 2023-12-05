// Objects representing a user created local database
use crate::cli::schema::Schema;
use chrono::prelude::*;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Database {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub schemas: Vec<Schema>,
}
