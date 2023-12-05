use chrono::prelude::*;
use serde::Deserialize;
use serde::Serialize;
use std::cmp::PartialEq;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct CloudAccount {
    pub name: Option<String>,
    pub username: Option<String>,
    pub clerk_id: Option<String>,
    pub organizations: Vec<String>,
    pub created_at: Option<DateTime<Utc>>,
}
