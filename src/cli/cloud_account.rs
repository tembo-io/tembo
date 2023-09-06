use serde::Deserialize;
use serde::Serialize;
use std::cmp::PartialEq;
use toml::value::Datetime;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct CloudAccount {
    pub name: Option<String>,
    pub username: Option<String>,
    pub created_at: Option<Datetime>,
}
